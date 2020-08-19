#include <stdlib.h>
#include <unistd.h>
#include <errno.h>
#include <sys/select.h>
#include "../include/hyper.h"


struct conn_data {
	int fd;
	hyper_waker *read_waker;
	hyper_waker *write_waker;
	struct conn_fds *all_fds;
};

struct conn_fds {
	fd_set read;
	fd_set write;
	fd_set excep;
};

static size_t read_cb(void *userdata, hyper_waker *waker, uint8_t *buf, size_t buf_len) {
	struct conn_data *conn = (struct conn_data *)userdata;
	size_t ret = read(conn->fd, buf, buf_len);

	if (ret < 0) {
		int err = errno;
		if (err == EAGAIN) {
			// would block, register interest
			if (conn->read_waker != NULL) {
				hyper_waker_free(conn->read_waker);
			}
			conn->read_waker = hyper_waker_clone(waker);
			FD_SET(conn->fd, &conn->all_fds->read);
			return HYPER_IO_PENDING;
		} else {
			// kaboom
			return HYPER_IO_ERROR;
		}
	} else {
		return ret;
	}
}

static size_t write_cb(void *userdata, hyper_waker *waker, const uint8_t *buf, size_t buf_len) {
	struct conn_data *conn = (struct conn_data *)userdata;
	size_t ret = write(conn->fd, buf, buf_len);

	if (ret < 0) {
		int err = errno;
		if (err == EAGAIN) {
			// would block, register interest
			if (conn->write_waker != NULL) {
				hyper_waker_free(conn->write_waker);
			}
			conn->write_waker = hyper_waker_clone(waker);
			FD_SET(conn->fd, &conn->all_fds->write);
			return HYPER_IO_PENDING;
		} else {
			// kaboom
			return HYPER_IO_ERROR;
		}
	} else {
		return ret;
	}
}

int main() {

	struct conn_fds *all_fds = malloc(sizeof(struct conn_fds));

	struct conn_data *conn = malloc(sizeof(struct conn_data));
	//TODO: YIKES
	// should do a connect() instead
	conn->fd = 3;
	conn->all_fds = all_fds;
	conn->read_waker = NULL;
	conn->write_waker = NULL;


	FD_ZERO(&all_fds->read);
	FD_ZERO(&all_fds->write);
	FD_ZERO(&all_fds->excep);

	// Hookup the IO
	hyper_io *io = hyper_io_new();
	hyper_io_set_data(io, (void *)conn);
	hyper_io_set_read(io, read_cb);
	hyper_io_set_write(io, write_cb);

	// Prepare client options
	hyper_clientconn_options *opts = hyper_clientconn_options_new();

	hyper_task *handshake = hyper_clientconn_handshake(io, opts);

	// We need an executor generally to poll futures
	hyper_executor *exec = hyper_executor_new();

	// Let's wait for the handshake to finish...
	hyper_executor_push(exec, handshake);

	// We're going to cheat for the handshake, since we know HTTP/1 handshakes
	// are immediately ready after the first poll.
	hyper_executor_poll(exec);
	hyper_task *task = hyper_executor_pop(exec);
	if (hyper_task_type(task) != HYPER_TASK_CLIENTCONN_HANDSHAKE) {
		// ruh roh!
		return 1;
	}

	hyper_clientconn *client = hyper_task_value(task);


	// Prepare the request
	hyper_request *req = hyper_request_new();
	if (!hyper_request_set_method(req, (uint8_t *)"POST", 4)) {
		return 1;
	}
	if (!hyper_request_set_uri(req, (uint8_t *)"http://httpbin.org", sizeof("http://httpbin.org") - 1)) {
		return 1;
	}

	// Send it!
	task = hyper_clientconn_send(client, req);

	hyper_executor_push(exec, task);

	// TODO: NEXT
	/*
	while (1) {
		hyper_executor_poll(exec);
		while (1) {
			hyper_task *task = hyper_executor_pop(exec);
			if (!task) {
				break;
			}
			switch (hyper_task_type(task)) {
			case HYPER_TASK_CLIENTCONN_HANDSHAKE:
				// Take the results
				client = hyper_task_value(task);
				break;
			default:
				hyper_task_free(task);
				break;
			}
		}

		select(1, &all_fds->read, &all_fds->write, &all_fds->excep, NULL);

	}
	*/
}
