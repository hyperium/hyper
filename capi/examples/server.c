#include <stdlib.h>
#include <stdio.h>
#include <unistd.h>
#include <fcntl.h>
#include <errno.h>
#include <assert.h>
#include <sys/epoll.h>

#include <sys/types.h>
#include <sys/socket.h>
#include <netdb.h>
#include <string.h>

#include "hyper.h"

static const int MAX_EVENTS = 128;

typedef struct conn_data_s {
    int fd;
    hyper_waker *read_waker;
    hyper_waker *write_waker;
} conn_data;

int epoll = -1;

static int listen_on(const char* host, const char* port) {
    int sock = socket(AF_INET, SOCK_STREAM, 0);
    int reuseaddr = 1;
    if (setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &reuseaddr, sizeof(int)) < 0) {
      perror("setsockopt");
    }
    struct sockaddr_in sin;
    sin.sin_family = AF_INET;
    sin.sin_port = htons(1234);
    sin.sin_addr.s_addr = 0;
    if (bind(sock, (struct sockaddr*)&sin, sizeof(struct sockaddr_in)) < 0) {
        perror("bind");
        return -1;
    }
    if (listen(sock, 5) < 0) {
        perror("listen");
        return -1;
    }

    return sock;
}

static size_t read_cb(void *userdata, hyper_context *ctx, uint8_t *buf, size_t buf_len) {
    conn_data *conn = (conn_data *)userdata;
    ssize_t ret = read(conn->fd, buf, buf_len);

    if (ret == 0) {
        // Closed
        return ret;
    }

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
    conn_data *conn = (conn_data *)userdata;
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

static conn_data* create_conn_data(int fd) {
    conn_data *conn = malloc(sizeof(conn_data));

    // Add fd to epoll set, associated with this `conn`
    struct epoll_event transport_event;
    transport_event.events = EPOLLIN;
    transport_event.data.ptr = conn;
    if (epoll_ctl(epoll, EPOLL_CTL_ADD, fd, &transport_event) < 0) {
        perror("epoll_ctl (transport)");
        free(conn);
        return NULL;
    }

    conn->fd = fd;
    conn->read_waker = NULL;
    conn->write_waker = NULL;

    return conn;
}

static hyper_io* create_io(conn_data* conn) {
    // Hookup the IO
    hyper_io *io = hyper_io_new();
    hyper_io_set_userdata(io, (void *)conn);
    hyper_io_set_read(io, read_cb);
    hyper_io_set_write(io, write_cb);

    return io;
}

static void free_conn_data(conn_data *conn) {
    // Disassociate with the epoll
    if (epoll_ctl(epoll, EPOLL_CTL_DEL, conn->fd, NULL) < 0) {
        perror("epoll_ctl (transport)");
    }

    close(conn->fd);

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

static void server_callback(void* userdata, hyper_request* request, hyper_response* response, hyper_response_channel* channel) {
    hyper_response_channel_send(channel, response);
}

#define STR_ARG(XX) (uint8_t *)XX, strlen(XX)

int main(int argc, char *argv[]) {
    const char *host = argc > 1 ? argv[1] : "127.0.0.1";
    const char *port = argc > 2 ? argv[2] : "1234";
    printf("listening on port %s on %s...\n", port, host);

    int listen_fd = listen_on(host, port);
    if (listen_fd < 0) {
        return 1;
    }

    if (fcntl(listen_fd, F_SETFL, O_NONBLOCK) != 0) {
        perror("fcntl(O_NONBLOCK) (listening)\n");
        return 1;
    }

    // Use epoll cos' it's cool
    epoll = epoll_create1(EPOLL_CLOEXEC);
    if (epoll < 0) {
        perror("epoll");
        return 1;
    }

    // Always await new connections from the listen socket
    struct epoll_event listen_event;
    listen_event.events = EPOLLIN;
    listen_event.data.ptr = NULL;
    if (epoll_ctl(epoll, EPOLL_CTL_ADD, listen_fd, &listen_event) < 0) {
        perror("epoll_crt (add listening)");
        return 1;
    }

    printf("http handshake (hyper v%s) ...\n", hyper_version());

    // We need an executor generally to poll futures
    const hyper_executor *exec = hyper_executor_new();

    // Might have an error
    hyper_error *err;

    while (1) {
        while (1) {
            hyper_task* task = hyper_executor_poll(exec);
            if (!task) {
                break;
            }
            printf("Task completed\n");

            if (hyper_task_type(task) == HYPER_TASK_ERROR) {
                printf("handshake error!\n");

                err = hyper_task_value(task);
                printf("error code: %d\n", hyper_error_code(err));
                uint8_t errbuf [256];
                size_t errlen = hyper_error_print(err, errbuf, sizeof(errbuf));
                printf("details: %.*s\n", (int) errlen, errbuf);

                // clean up the error
                hyper_error_free(err);

                // clean up the task
                conn_data* conn = hyper_task_userdata(task);
                if (conn) {
                    free_conn_data(conn);
                }
                hyper_task_free(task);

                continue;
            }

            if (hyper_task_type(task) == HYPER_TASK_EMPTY) {
                conn_data* conn = hyper_task_userdata(task);
                if (conn) {
                    printf("server connection complete\n");
                    free_conn_data(conn);
                } else {
                    printf("internal hyper task complete\n");
                }
                hyper_task_free(task);

                continue;
            }
        }

        printf("Processed all tasks - polling for events\n");

        struct epoll_event events[MAX_EVENTS];

        int nevents = epoll_wait(epoll, events, MAX_EVENTS, -1);
        if (nevents < 0) {
            perror("epoll");
            return 1;
        }

        printf("Poll reported %d events\n", nevents);

        for (int n = 0; n < nevents; n++) {
            if (events[n].data.ptr == NULL) {
                // Incoming connection(s) on listen_fd
                int new_fd;
                while ((new_fd = accept(listen_fd, NULL, 0)) >= 0) {
                  printf("New incoming connection\n");

                  // Set non-blocking
                  if (fcntl(new_fd, F_SETFL, O_NONBLOCK) != 0) {
                      perror("fcntl(O_NONBLOCK) (listening)\n");
                      return 1;
                  }

                  // Wire up IO
                  conn_data *conn = create_conn_data(new_fd);
                  hyper_io* io = create_io(conn);

                  // Ask hyper to drive this connection
                  hyper_serverconn_options *opts = hyper_serverconn_options_new(exec);
                  hyper_service *service = hyper_service_new(server_callback);
                  hyper_task *serverconn = hyper_serve_connection(opts, io, service);
                  hyper_task_set_userdata(serverconn, conn);
                  hyper_executor_push(exec, serverconn);
                }

                if (errno != EAGAIN) {
                  perror("accept");
                }
            } else {
                // Existing transport socket, poke the wakers or close the socket
                conn_data* conn = events[n].data.ptr;
                if ((events[n].events & EPOLLIN) && conn->read_waker) {
                    hyper_waker_wake(conn->read_waker);
                    conn->read_waker = NULL;
                }
                if ((events[n].events & EPOLLOUT) && conn->write_waker) {
                    hyper_waker_wake(conn->write_waker);
                    conn->write_waker = NULL;
                }
            }
        }
    }

    return 1;
}
