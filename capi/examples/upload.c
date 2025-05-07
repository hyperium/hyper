#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <sys/select.h>
#include <sys/socket.h>

#include "hyper.h"

struct conn_data {
    int fd;
    hyper_waker *read_waker;
    hyper_waker *write_waker;
};

static size_t read_cb(void *userdata, hyper_context *ctx, uint8_t *buf, size_t buf_len) {
    struct conn_data *conn = (struct conn_data *)userdata;
    ssize_t ret = read(conn->fd, buf, buf_len);

    if (ret >= 0) {
        return ret;
    }

    if (errno != EAGAIN) {
        // kaboom
        return HYPER_IO_ERROR;
    }

    // would block, register interest
    if (conn->read_waker != NULL) {
        hyper_waker_free(conn->read_waker);
    }
    conn->read_waker = hyper_context_waker(ctx);
    return HYPER_IO_PENDING;
}

static size_t write_cb(void *userdata, hyper_context *ctx, const uint8_t *buf, size_t buf_len) {
    struct conn_data *conn = (struct conn_data *)userdata;
    ssize_t ret = write(conn->fd, buf, buf_len);

    if (ret >= 0) {
        return ret;
    }

    if (errno != EAGAIN) {
        // kaboom
        return HYPER_IO_ERROR;
    }

    // would block, register interest
    if (conn->write_waker != NULL) {
        hyper_waker_free(conn->write_waker);
    }
    conn->write_waker = hyper_context_waker(ctx);
    return HYPER_IO_PENDING;
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
        }

        close(sfd);
    }

    freeaddrinfo(result);

    // no address succeeded
    if (rp == NULL) {
        printf("connect failed for %s\n", host);
        return -1;
    }

    return sfd;
}

struct upload_body {
    int fd;
    unsigned char *buf;
    size_t len;
};

static int poll_req_upload(void *userdata, hyper_context *ctx, hyper_buf **chunk) {
    struct upload_body *upload = userdata;

    ssize_t res = read(upload->fd, upload->buf, upload->len);
    if (res > 0) {
        *chunk = hyper_buf_copy(upload->buf, res);
        return HYPER_POLL_READY;
    }

    if (res == 0) {
        // All done!
        *chunk = NULL;
        return HYPER_POLL_READY;
    }

    // Oh no!
    printf("error reading upload file: %d", errno);
    return HYPER_POLL_ERROR;
}

static int print_each_header(
    void *userdata, const uint8_t *name, size_t name_len, const uint8_t *value, size_t value_len
) {
    printf("%.*s: %.*s\n", (int)name_len, name, (int)value_len, value);
    return HYPER_ITER_CONTINUE;
}

static void print_informational(void *userdata, hyper_response *resp) {
    uint16_t http_status = hyper_response_status(resp);

    printf("\nInformational (1xx): %d\n", http_status);
}

typedef enum {
    EXAMPLE_NOT_SET = 0, // tasks we don't know about won't have a userdata set
    EXAMPLE_HANDSHAKE,
    EXAMPLE_SEND,
    EXAMPLE_RESP_BODY
} example_state;

typedef union example_id {
    void *ptr;
    example_state state;
} example_userdata;

#define STR_ARG(XX) (uint8_t *)XX, strlen(XX)

int main(int argc, char *argv[]) {
    const char *file = argc > 1 ? argv[1] : NULL;
    const char *host = argc > 2 ? argv[2] : "httpbin.org";
    const char *port = argc > 3 ? argv[3] : "80";
    const char *path = argc > 4 ? argv[4] : "/post";

    if (!file) {
        printf("Pass a file path as the first argument.\n");
        return 1;
    }

    struct upload_body upload;
    upload.fd = open(file, O_RDONLY);

    if (upload.fd < 0) {
        printf("error opening file to upload: %s\n", strerror(errno));
        return 1;
    }
    printf("connecting to port %s on %s...\n", port, host);

    int fd = connect_to(host, port);
    if (fd < 0) {
        return 1;
    }
    printf("connected to %s, now upload to %s\n", host, path);

    if (fcntl(fd, F_SETFL, O_NONBLOCK) != 0) {
        printf("failed to set socket to non-blocking\n");
        return 1;
    }

    upload.len = 8192;
    upload.buf = malloc(upload.len);

    fd_set fds_read;
    fd_set fds_write;
    fd_set fds_excep;

    struct conn_data *conn = malloc(sizeof(struct conn_data));

    conn->fd = fd;
    conn->read_waker = NULL;
    conn->write_waker = NULL;

    // Hookup the IO
    hyper_io *io = hyper_io_new();
    hyper_io_set_userdata(io, (void *)conn, NULL);
    hyper_io_set_read(io, read_cb);
    hyper_io_set_write(io, write_cb);

    printf("http handshake (hyper v%s) ...\n", hyper_version());

    // We need an executor generally to poll futures
    const hyper_executor *exec = hyper_executor_new();

    // Prepare client options
    hyper_clientconn_options *opts = hyper_clientconn_options_new();
    hyper_clientconn_options_exec(opts, exec);

    hyper_task *handshake = hyper_clientconn_handshake(io, opts);
    hyper_task_set_userdata(handshake, (void *)EXAMPLE_HANDSHAKE, NULL);

    // Let's wait for the handshake to finish...
    hyper_executor_push(exec, handshake);

    // This body will get filled in eventually...
    hyper_body *resp_body = NULL;

    // The polling state machine!
    while (1) {
        // Poll all ready tasks and act on them...
        while (1) {
            hyper_task *task = hyper_executor_poll(exec);
            if (!task) {
                break;
            }
            hyper_task_return_type task_type = hyper_task_type(task);

            switch (((example_userdata)hyper_task_userdata(task)).state) {
            case EXAMPLE_HANDSHAKE:
                if (task_type == HYPER_TASK_ERROR) {
                    printf("handshake error!\n");
                    return 1;
                }
                assert(task_type == HYPER_TASK_CLIENTCONN);

                printf("preparing http request ...\n");

                hyper_clientconn *client = hyper_task_value(task);
                hyper_task_free(task);

                // Prepare the request
                hyper_request *req = hyper_request_new();
                if (hyper_request_set_method(req, STR_ARG("POST"))) {
                    printf("error setting method\n");
                    return 1;
                }
                if (hyper_request_set_uri(req, STR_ARG(path))) {
                    printf("error setting uri\n");
                    return 1;
                }

                hyper_headers *req_headers = hyper_request_headers(req);
                hyper_headers_set(req_headers, STR_ARG("host"), STR_ARG(host));
                hyper_headers_set(req_headers, STR_ARG("expect"), STR_ARG("100-continue"));

                // NOTE: We aren't handling *waiting* for the 100 Continue,
                // the body is sent immediately. This will just print if any
                // informational headers are received.
                printf("    with expect-continue ...\n");
                hyper_request_on_informational(req, print_informational, NULL, NULL);

                // Prepare the req body
                hyper_body *body = hyper_body_new();
                hyper_body_set_userdata(body, &upload, NULL);
                hyper_body_set_data_func(body, poll_req_upload);
                hyper_request_set_body(req, body);

                // Send it!
                hyper_task *send = hyper_clientconn_send(client, req);
                hyper_task_set_userdata(send, (void *)EXAMPLE_SEND, NULL);
                printf("sending ...\n");
                hyper_executor_push(exec, send);

                // For this example, no longer need the client
                hyper_clientconn_free(client);

                break;
            case EXAMPLE_SEND:
                if (task_type == HYPER_TASK_ERROR) {
                    printf("send error!\n");
                    return 1;
                }
                assert(task_type == HYPER_TASK_RESPONSE);

                // Take the results
                hyper_response *resp = hyper_task_value(task);
                hyper_task_free(task);

                uint16_t http_status = hyper_response_status(resp);

                printf("\nResponse Status: %d\n", http_status);

                hyper_headers *headers = hyper_response_headers(resp);
                hyper_headers_foreach(headers, print_each_header, NULL);
                printf("\n");

                resp_body = hyper_response_body(resp);

                // Set us up to peel data from the body a chunk at a time
                hyper_task *body_data = hyper_body_data(resp_body);
                hyper_task_set_userdata(body_data, (void *)EXAMPLE_RESP_BODY, NULL);
                hyper_executor_push(exec, body_data);

                // No longer need the response
                hyper_response_free(resp);

                break;
            case EXAMPLE_RESP_BODY:
                if (task_type == HYPER_TASK_ERROR) {
                    printf("body error!\n");
                    return 1;
                }

                if (task_type == HYPER_TASK_BUF) {
                    hyper_buf *chunk = hyper_task_value(task);
                    write(1, hyper_buf_bytes(chunk), hyper_buf_len(chunk));
                    hyper_buf_free(chunk);
                    hyper_task_free(task);

                    hyper_task *body_data = hyper_body_data(resp_body);
                    hyper_task_set_userdata(body_data, (void *)EXAMPLE_RESP_BODY, NULL);
                    hyper_executor_push(exec, body_data);

                    break;
                }

                assert(task_type == HYPER_TASK_EMPTY);
                hyper_task_free(task);
                hyper_body_free(resp_body);

                printf("\n -- Done! -- \n");

                // Cleaning up before exiting
                hyper_executor_free(exec);
                free_conn_data(conn);
                free(upload.buf);

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
        }

        if (FD_ISSET(conn->fd, &fds_read)) {
            hyper_waker_wake(conn->read_waker);
            conn->read_waker = NULL;
        }

        if (FD_ISSET(conn->fd, &fds_write)) {
            hyper_waker_wake(conn->write_waker);
            conn->write_waker = NULL;
        }
    }

    return 0;
}
