#include <stdlib.h>
#include <stdio.h>
#include <unistd.h>
#include <fcntl.h>
#include <errno.h>
#include <sys/select.h>
#include <assert.h>

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

static void free_conn_data(struct conn_data *conn) {
    if (conn->read_waker) {
        hyper_waker_free(conn->read_waker);
        conn->read_waker = NULL;
    }
    if (conn->write_waker) {
        hyper_waker_free(conn->write_waker);
        conn->write_waker = NULL;
    }

    free(conn);
}

static int connect_to(const char *host, const char *port) {
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

static int print_each_header(void *userdata,
                                         const uint8_t *name,
                                         size_t name_len,
                                         const uint8_t *value,
                                         size_t value_len) {
    printf("%.*s: %.*s\n", (int) name_len, name, (int) value_len, value);
    return HYPER_ITER_CONTINUE;
}

static int print_each_chunk(void *userdata, const hyper_buf *chunk) {
    const uint8_t *buf = hyper_buf_bytes(chunk);
    size_t len = hyper_buf_len(chunk);

    write(1, buf, len);

    return HYPER_ITER_CONTINUE;
}

typedef enum {
    EXAMPLE_NOT_SET = 0, // tasks we don't know about won't have a userdata set
    EXAMPLE_HANDSHAKE,
    EXAMPLE_SEND,
    EXAMPLE_RESP_BODY
} example_id;

#define STR_ARG(XX) (uint8_t *)XX, strlen(XX)

int main(int argc, char *argv[]) {
        const char *host = argc > 1 ? argv[1] : "httpbin.org";
        const char *port = argc > 2 ? argv[2] : "80";
        const char *path = argc > 3 ? argv[3] : "/";
        printf("connecting to port %s on %s...\n", port, host);

        int fd = connect_to(host, port);
    if (fd < 0) {
        return 1;
    }
        printf("connected to %s, now get %s\n", host, path);

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
    hyper_io_set_userdata(io, (void *)conn);
    hyper_io_set_read(io, read_cb);
    hyper_io_set_write(io, write_cb);

    printf("http handshake ...\n");

    // We need an executor generally to poll futures
    const hyper_executor *exec = hyper_executor_new();

    // Prepare client options
    hyper_clientconn_options *opts = hyper_clientconn_options_new();
    hyper_clientconn_options_exec(opts, exec);

    hyper_task *handshake = hyper_clientconn_handshake(io, opts);
    hyper_task_set_userdata(handshake, (void *)EXAMPLE_HANDSHAKE);

    // Let's wait for the handshake to finish...
    hyper_executor_push(exec, handshake);

    // In case a task errors...
    hyper_error *err;

    // The polling state machine!
    while (1) {
        // Poll all ready tasks and act on them...
        while (1) {
            hyper_task *task = hyper_executor_poll(exec);
            if (!task) {
                break;
            }
            switch ((example_id) hyper_task_userdata(task)) {
            case EXAMPLE_HANDSHAKE:
                ;
                if (hyper_task_type(task) == HYPER_TASK_ERROR) {
                    printf("handshake error!\n");
                    err = hyper_task_value(task);
                    goto fail;
                }
                assert(hyper_task_type(task) == HYPER_TASK_CLIENTCONN);

                printf("preparing http request ...\n");

                hyper_clientconn *client = hyper_task_value(task);
                hyper_task_free(task);

                // Prepare the request
                hyper_request *req = hyper_request_new();
                if (hyper_request_set_method(req, STR_ARG("GET"))) {
                    printf("error setting method\n");
                    return 1;
                }
                if (hyper_request_set_uri(req, STR_ARG(path))) {
                    printf("error setting uri\n");
                    return 1;
                }

                hyper_headers *req_headers = hyper_request_headers(req);
                hyper_headers_set(req_headers,  STR_ARG("Host"), STR_ARG(host));

                // Send it!
                hyper_task *send = hyper_clientconn_send(client, req);
                hyper_task_set_userdata(send, (void *)EXAMPLE_SEND);
                printf("sending ...\n");
                hyper_executor_push(exec, send);

                // For this example, no longer need the client
                hyper_clientconn_free(client);

                break;
            case EXAMPLE_SEND:
                ;
                if (hyper_task_type(task) == HYPER_TASK_ERROR) {
                    printf("send error!\n");
                    err = hyper_task_value(task);
                    goto fail;
                }
                assert(hyper_task_type(task) == HYPER_TASK_RESPONSE);

                // Take the results
                hyper_response *resp = hyper_task_value(task);
                hyper_task_free(task);

                uint16_t http_status = hyper_response_status(resp);
                const uint8_t *rp = hyper_response_reason_phrase(resp);
                size_t rp_len = hyper_response_reason_phrase_len(resp);

                printf("\nResponse Status: %d %.*s\n", http_status, (int) rp_len, rp);

                hyper_headers *headers = hyper_response_headers(resp);
                hyper_headers_foreach(headers, print_each_header, NULL);
                printf("\n");

                hyper_body *resp_body = hyper_response_body(resp);
                hyper_task *foreach = hyper_body_foreach(resp_body, print_each_chunk, NULL);
                hyper_task_set_userdata(foreach, (void *)EXAMPLE_RESP_BODY);
                hyper_executor_push(exec, foreach);

                // No longer need the response
                hyper_response_free(resp);

                break;
            case EXAMPLE_RESP_BODY:
                ;
                if (hyper_task_type(task) == HYPER_TASK_ERROR) {
                    printf("body error!\n");
                    err = hyper_task_value(task);
                    goto fail;
                }

                assert(hyper_task_type(task) == HYPER_TASK_EMPTY);

                printf("\n -- Done! -- \n");

                // Cleaning up before exiting
                hyper_task_free(task);
                hyper_executor_free(exec);
                free_conn_data(conn);

                return 0;
            case EXAMPLE_NOT_SET:
                // A background task for hyper completed...
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

        int sel_ret = select(conn->fd + 1, &fds_read, &fds_write, &fds_excep, NULL);

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

fail:
    if (err) {
        printf("error code: %d\n", hyper_error_code(err));
        // grab the error details
        char errbuf [256];
        size_t errlen = hyper_error_print(err, errbuf, sizeof(errbuf));
        printf("details: %.*s\n", (int) errlen, errbuf);

        // clean up the error
        hyper_error_free(err);
    }
    return 1;
}
