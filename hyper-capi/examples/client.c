#include <stdlib.h>
#include <stdio.h>
#include <unistd.h>
#include <fcntl.h>
#include <errno.h>
#include <sys/select.h>

#include <sys/types.h>
#include <sys/socket.h>
#include <netdb.h>
#include <string.h>

#include "hyper.h"


struct conn_data {
	int fd;
	hyper_waker *read_waker;
	hyper_waker *write_waker;
};

static size_t read_cb(void *userdata, hyper_context *ctx, uint8_t *buf, size_t buf_len) {
	struct conn_data *conn = (struct conn_data *)userdata;
	ssize_t ret = read(conn->fd, buf, buf_len);

	if (ret < 0) {
		int err = errno;
		if (err == EAGAIN) {
			// would block, register interest
			if (conn->read_waker != NULL) {
				hyper_waker_free(conn->read_waker);
			}
			conn->read_waker = hyper_context_waker(ctx);
			return HYPER_IO_PENDING;
		} else {
			// kaboom
			return HYPER_IO_ERROR;
		}
	} else {
		return ret;
	}
}

static size_t write_cb(void *userdata, hyper_context *ctx, const uint8_t *buf, size_t buf_len) {
	struct conn_data *conn = (struct conn_data *)userdata;
	ssize_t ret = write(conn->fd, buf, buf_len);

	if (ret < 0) {
		int err = errno;
		if (err == EAGAIN) {
			// would block, register interest
			if (conn->write_waker != NULL) {
				hyper_waker_free(conn->write_waker);
			}
			conn->write_waker = hyper_context_waker(ctx);
			return HYPER_IO_PENDING;
		} else {
			// kaboom
			return HYPER_IO_ERROR;
		}
	} else {
		return ret;
	}
}

static int connect_to(char *host, char *port) {
	struct addrinfo hints;
	memset(&hints, 0, sizeof(struct addrinfo));
	hints.ai_family = AF_UNSPEC;
	hints.ai_socktype = SOCK_STREAM;

	struct addrinfo *result, *rp;
	if (getaddrinfo(host, port, &hints, &result) != 0) {
		printf("dns failed for %s\n", host);
		return -1;
	}

	int sfd;
	for (rp = result; rp != NULL; rp = rp->ai_next) {
		sfd = socket(rp->ai_family, rp->ai_socktype, rp->ai_protocol);
		if (sfd == -1) {
			continue;
		}

		if (connect(sfd, rp->ai_addr, rp->ai_addrlen) != -1) {
			break;
		} else {
			close(sfd);
		}
	}

	freeaddrinfo(result);

	// no address succeeded
	if (rp == NULL) {
		printf("connect failed for %s\n", host);
		return -1;
	}

	return sfd;
}

static hyper_iter_step print_each_header(void *userdata,
                                         const uint8_t *name,
                                         size_t name_len,
                                         const uint8_t *value,
                                         size_t value_len) {
	printf("%.*s: %.*s\n", (int) name_len, name, (int) value_len, value);
	return HYPER_IT_CONTINUE;
}

int main(int argc, char *argv[]) {
	printf("connecting ...\n");

	int fd = connect_to("httpbin.org", "80");
	if (fd < 0) {
		return 1;
	}
	printf("connected to httpbin.org\n");

	if (fcntl(fd, F_SETFL, O_NONBLOCK) != 0) {
		printf("failed to set socket to non-blocking\n");
		return 1;
	}

	fd_set fds_read;
	fd_set fds_write;
	fd_set fds_excep;

	struct conn_data *conn = malloc(sizeof(struct conn_data));

	conn->fd = fd;
	conn->read_waker = NULL;
	conn->write_waker = NULL;


	// Hookup the IO
	hyper_io *io = hyper_io_new();
	hyper_io_set_data(io, (void *)conn);
	hyper_io_set_read(io, read_cb);
	hyper_io_set_write(io, write_cb);

	printf("http handshake ...\n");

	// We need an executor generally to poll futures
	hyper_executor *exec = hyper_executor_new();

	// Prepare client options
	hyper_clientconn_options *opts = hyper_clientconn_options_new();
	hyper_clientconn_options_exec(opts, exec);

	hyper_task *handshake = hyper_clientconn_handshake(io, opts);


	// Let's wait for the handshake to finish...
	hyper_executor_push(exec, handshake);

	// We're going to cheat for the handshake, since we know HTTP/1 handshakes
	// are immediately ready after the first poll.
	hyper_task *task = hyper_executor_poll(exec);
	if (hyper_task_type(task) != HYPER_TASK_CLIENTCONN) {
		// ruh roh!
		printf("task not a handshake ?!\n");
		return 1;
	}

	printf("preparing http request ...\n");

	hyper_clientconn *client = hyper_task_value(task);


	// Prepare the request
	hyper_request *req = hyper_request_new();
	if (hyper_request_set_method(req, (uint8_t *)"GET", 3)) {
		printf("error setting method\n");
		return 1;
	}
	if (hyper_request_set_uri(req, (uint8_t *)"/", sizeof("/") - 1)) {
		printf("error setting uri\n");
		return 1;
	}

	hyper_headers *req_headers = hyper_request_headers(req);
	hyper_headers_set(req_headers,  (uint8_t *)"host", 4, (uint8_t *)"httpbin.org", sizeof("httpbin.org") - 1);

	// Send it!
	task = hyper_clientconn_send(client, req);

	printf("sending ...\n");

	hyper_executor_push(exec, task);

	// The body will be filled in after we get a response, and we will poll it
	// multiple times, so it's declared out here.
	//hyper_body *resp_body = NULL;

	int sel_ret;

	// The polling state machine!
	while (1) {
		while (1) {
			hyper_task *task = hyper_executor_poll(exec);
			if (!task) {
				break;
			}
			switch (hyper_task_type(task)) {
			case HYPER_TASK_RESPONSE:
				;
				// Take the results
				hyper_response *resp = hyper_task_value(task);
				hyper_task_free(task);

				uint16_t http_status = hyper_response_status(resp);
				
				printf("\nResponse Status: %d\n", http_status);

				hyper_headers *headers = hyper_response_headers(resp);
				hyper_headers_foreach(headers, print_each_header, NULL);

				printf("\n -- Done! --\n");
				return 0;
			/*
				resp_body = hyper_response_body(resp);
				hyper_executor_push(exec, hyper_body_next(resp_body));
				
				break;
			case HYPER_TASK_BODY_NEXT:
				;
				// A body chunk is available
				hyper_buf *chunk = hyper_task_value(task);
				hyper_task_free(task);

				if (!chunk) {
					// body is complete!
					printf("\n\nDone!");
					return 0;
				}

				// Write the chunk to stdout
				hyper_str s = hyper_buf_str(chunk);
				write(1, s.buf, s.len);
				hyper_buf_free(chunk);
				
				// Queue up another body poll.
				hyper_executor_push(exec, hyper_body_next(resp_body));
				break;
				*/
			case HYPER_TASK_ERROR:
				printf("task error!\n");
				return 1;
			default:
				hyper_task_free(task);
				break;
			}
		}

		// All futures are pending on IO work, so select on the fds.

		FD_ZERO(&fds_read);
		FD_ZERO(&fds_write);
		FD_ZERO(&fds_excep);

		if (conn->read_waker) {
			FD_SET(conn->fd, &fds_read);
		}
		if (conn->write_waker) {
			FD_SET(conn->fd, &fds_write);
		}

		sel_ret = select(conn->fd + 1, &fds_read, &fds_write, &fds_excep, NULL);

		if (sel_ret < 0) {
			printf("select() error\n");
			return 1;
		} else {
			if (FD_ISSET(conn->fd, &fds_read)) {
				hyper_waker_wake(conn->read_waker);
				conn->read_waker = NULL;
			}
			if (FD_ISSET(conn->fd, &fds_write)) {
				hyper_waker_wake(conn->write_waker);
				conn->write_waker = NULL;
			}
		}

	}


	return 0;
}
