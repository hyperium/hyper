#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include <sys/epoll.h>
#include <sys/signalfd.h>
#include <sys/socket.h>

#include "hyper.h"

static const int MAX_EVENTS = 128;

typedef struct conn_data_s {
    int fd;
    int epoll_fd;
    uint32_t event_mask;
    hyper_waker *read_waker;
    hyper_waker *write_waker;
} conn_data;

static int listen_on(const char *host, const char *port) {
    struct addrinfo hints;
    struct addrinfo *result;

    // Work out bind address
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_flags = AI_PASSIVE;
    hints.ai_protocol = 0;
    hints.ai_canonname = NULL;
    hints.ai_addr = NULL;
    hints.ai_next = NULL;

    int gai_rc = getaddrinfo(host, port, &hints, &result);
    if (gai_rc != 0) {
        fprintf(stderr, "getaddrinfo: %s\n", gai_strerror(gai_rc));
        return -1;
    }

    // Try each bind address until one works
    int sock = -1;
    for (struct addrinfo *resp = result; resp; resp = resp->ai_next) {
        sock = socket(resp->ai_family, resp->ai_socktype, resp->ai_protocol);
        if (sock < 0) {
            perror("socket");
            continue;
        }

        // Enable SO_REUSEADDR
        int reuseaddr = 1;
        if (setsockopt(sock, SOL_SOCKET, SO_REUSEADDR, &reuseaddr, sizeof(int)) < 0) {
            perror("setsockopt");
        }

        // Attempt to bind to the address
        if (bind(sock, resp->ai_addr, resp->ai_addrlen) == 0) {
            break;
        }
        perror("bind");

        // Failed, tidy up
        close(sock);
        sock = -1;
    }

    freeaddrinfo(result);

    if (sock < 0) {
        return -1;
    }

    // Non-blocking for async
    if (fcntl(sock, F_SETFL, O_NONBLOCK) != 0) {
        perror("fcntl(O_NONBLOCK) (listening)\n");
        return -1;
    }

    // Close handle on exec(ve)
    if (fcntl(sock, F_SETFD, FD_CLOEXEC) != 0) {
        perror("fcntl(FD_CLOEXEC) (listening)\n");
        return 1;
    }

    // Enable listening mode
    if (listen(sock, 32) < 0) {
        perror("listen");
        return -1;
    }

    return sock;
}

// Register interest in various termination signals.  The returned fd can be
// polled with epoll.
static int register_signal_handler() {
    sigset_t mask;
    sigemptyset(&mask);
    sigaddset(&mask, SIGINT);
    sigaddset(&mask, SIGTERM);
    sigaddset(&mask, SIGQUIT);
    int signal_fd = signalfd(-1, &mask, SFD_NONBLOCK | SFD_CLOEXEC);
    if (signal_fd < 0) {
        perror("signalfd");
        return 1;
    }
    sigaddset(&mask, SIGPIPE);
    if (sigprocmask(SIG_BLOCK, &mask, NULL) < 0) {
        perror("sigprocmask");
        return 1;
    }

    return signal_fd;
}

// Register connection FD with epoll, associated with this `conn`
static bool update_conn_data_registrations(conn_data* conn, bool create) {
    struct epoll_event transport_event;
    transport_event.events = conn->event_mask;
    transport_event.data.ptr = conn;
    if (epoll_ctl(conn->epoll_fd, create ? EPOLL_CTL_ADD : EPOLL_CTL_MOD, conn->fd, &transport_event) < 0) {
        perror("epoll_ctl (transport)");
        return false;
    } else {
        return true;
    }
}

static size_t read_cb(void *userdata, hyper_context *ctx, uint8_t *buf, size_t buf_len) {
    conn_data *conn = (conn_data *)userdata;
    ssize_t ret = read(conn->fd, buf, buf_len);

    if (ret >= 0) {
        // Normal (synchronous) read successful (or socket is closed)
        return ret;
    }

    if (errno != EAGAIN) {
        // kaboom
        return HYPER_IO_ERROR;
    }

    // Otherwise this would block, so register interest and return pending
    if (conn->read_waker != NULL) {
        hyper_waker_free(conn->read_waker);
    }

    if (!(conn->event_mask & EPOLLIN)) {
      conn->event_mask |= EPOLLIN;
      if (!update_conn_data_registrations(conn, false)) {
        return HYPER_IO_ERROR;
      }
    }

    conn->read_waker = hyper_context_waker(ctx);
    return HYPER_IO_PENDING;
}

static size_t write_cb(void *userdata, hyper_context *ctx, const uint8_t *buf, size_t buf_len) {
    conn_data *conn = (conn_data *)userdata;
    ssize_t ret = write(conn->fd, buf, buf_len);

    if (ret >= 0) {
        // Normal (synchronous) write successful (or socket is closed)
        return ret;
    }

    if (errno != EAGAIN) {
        // kaboom
        return HYPER_IO_ERROR;
    }

    // Otherwise this would block, so register interest and return pending
    if (conn->write_waker != NULL) {
        hyper_waker_free(conn->write_waker);
    }

    if (!(conn->event_mask & EPOLLOUT)) {
      conn->event_mask |= EPOLLOUT;
      if (!update_conn_data_registrations(conn, false)) {
        return HYPER_IO_ERROR;
      }
    }

    conn->write_waker = hyper_context_waker(ctx);
    return HYPER_IO_PENDING;
}

static conn_data *create_conn_data(int epoll, int fd) {
    conn_data *conn = malloc(sizeof(conn_data));
    conn->fd = fd;
    conn->epoll_fd = epoll;
    conn->event_mask = 0;
    conn->read_waker = NULL;
    conn->write_waker = NULL;

    if (!update_conn_data_registrations(conn, true)) {
      free(conn);
      return NULL;
    }

    return conn;
}

static void free_conn_data(void *userdata) {
    conn_data* conn = (conn_data*)userdata;

    // Disassociate with the epoll
    if (epoll_ctl(conn->epoll_fd, EPOLL_CTL_DEL, conn->fd, NULL) < 0) {
        perror("epoll_ctl (transport, delete)");
    }

    // Drop any saved-off wakers
    if (conn->read_waker) {
        hyper_waker_free(conn->read_waker);
        conn->read_waker = NULL;
    }
    if (conn->write_waker) {
        hyper_waker_free(conn->write_waker);
        conn->write_waker = NULL;
    }

    // Shut down the socket connection
    close(conn->fd);

    // ...and clean up
    free(conn);
}

static hyper_io *create_io(conn_data *conn) {
    // Hookup the IO
    hyper_io *io = hyper_io_new();
    hyper_io_set_userdata(io, (void *)conn, free_conn_data);
    hyper_io_set_read(io, read_cb);
    hyper_io_set_write(io, write_cb);

    return io;
}

typedef struct service_userdata_s {
  char host[128];
  char port[8];
} service_userdata;

static service_userdata* create_service_userdata() {
  return (service_userdata*)calloc(1, sizeof(service_userdata));
}

static void free_service_userdata(void* userdata) {
  service_userdata* cast_userdata = (service_userdata*)userdata;
  free(cast_userdata);
}

static int print_each_header(
    void *userdata, const uint8_t *name, size_t name_len, const uint8_t *value, size_t value_len
) {
    printf("%.*s: %.*s\n", (int)name_len, name, (int)value_len, value);
    return HYPER_ITER_CONTINUE;
}

static void server_callback(
    void *userdata, hyper_request *request, hyper_response_channel *channel
) {
    service_userdata* service_data = (service_userdata*)userdata;
    printf("Request from %s:%s\n", service_data->host, service_data->port);

    // Print out various properties of the request.
    unsigned char scheme[16];
    size_t scheme_len = sizeof(scheme);
    unsigned char authority[16];
    size_t authority_len = sizeof(authority);
    unsigned char path_and_query[16];
    size_t path_and_query_len = sizeof(path_and_query);
    if (hyper_request_uri_parts(
            request,
            scheme,
            &scheme_len,
            authority,
            &authority_len,
            path_and_query,
            &path_and_query_len
        ) == 0) {
        printf("Request scheme was %.*s\n", (int)scheme_len, scheme);
        printf("Request authority was %.*s\n", (int)authority_len, authority);
        printf("Request path_and_query was %.*s\n", (int)path_and_query_len, path_and_query);
    }
    int version = hyper_request_version(request);
    printf("Request version was %d\n", version);
    unsigned char method[16];
    size_t method_len = sizeof(method);
    if (hyper_request_method(request, method, &method_len) == 0) {
        printf("Request method was %.*s\n", (int)method_len, method);
    }

    // Print out all the headers from the request
    hyper_headers *req_headers = hyper_request_headers(request);
    hyper_headers_foreach(req_headers, print_each_header, NULL);
    hyper_request_free(request);

    // Build a response
    hyper_response *response = hyper_response_new();
    hyper_response_set_status(response, 404);
    hyper_headers* rsp_headers = hyper_response_headers(response);
    hyper_headers_set(
        rsp_headers,
        (unsigned char*)"Cache-Control",
        13,
        (unsigned char*)"no-cache",
        8
    );

    // And send the response, completing the transaction
    hyper_response_channel_send(channel, response);
}

int main(int argc, char *argv[]) {
    const char *host = argc > 1 ? argv[1] : "127.0.0.1";
    const char *port = argc > 2 ? argv[2] : "1234";
    printf("listening on port %s on %s...\n", port, host);

    // The main listening socket
    int listen_fd = listen_on(host, port);
    if (listen_fd < 0) {
        return 1;
    }

    int signal_fd = register_signal_handler();
    if (signal_fd < 0) {
        return 1;
    }

    // Use epoll cos' it's cool
    int epoll = epoll_create1(EPOLL_CLOEXEC);
    if (epoll < 0) {
        perror("epoll");
        return 1;
    }

    // Always await new connections from the listen socket
    struct epoll_event listen_event;
    listen_event.events = EPOLLIN;
    listen_event.data.ptr = &listen_fd;
    if (epoll_ctl(epoll, EPOLL_CTL_ADD, listen_fd, &listen_event) < 0) {
        perror("epoll_ctl (add listening)");
        return 1;
    }

    // Always await signals on the signal socket
    struct epoll_event signal_event;
    signal_event.events = EPOLLIN;
    signal_event.data.ptr = &signal_fd;
    if (epoll_ctl(epoll, EPOLL_CTL_ADD, signal_fd, &signal_event) < 0) {
        perror("epoll_ctl (add signal)");
        return 1;
    }

    printf("http handshake (hyper v%s) ...\n", hyper_version());

    // We need an executor generally to poll futures
    const hyper_executor *exec = hyper_executor_new();

    // Configure the server HTTP/1 stack
    hyper_http1_serverconn_options *http1_opts = hyper_http1_serverconn_options_new(exec);
    hyper_http1_serverconn_options_header_read_timeout(http1_opts, 1000 * 5); // 5 seconds

    // Configure the server HTTP/2 stack
    hyper_http2_serverconn_options *http2_opts = hyper_http2_serverconn_options_new(exec);
    hyper_http2_serverconn_options_keep_alive_interval(http2_opts, 5); // 5 seconds
    hyper_http2_serverconn_options_keep_alive_timeout(http2_opts, 5); // 5 seconds

    while (1) {
        while (1) {
            hyper_task *task = hyper_executor_poll(exec);
            if (!task) {
                break;
            }

            if (hyper_task_type(task) == HYPER_TASK_ERROR) {
                printf("hyper task failed with error!\n");

                hyper_error* err = hyper_task_value(task);
                printf("error code: %d\n", hyper_error_code(err));
                uint8_t errbuf[256];
                size_t errlen = hyper_error_print(err, errbuf, sizeof(errbuf));
                printf("details: %.*s\n", (int)errlen, errbuf);

                // clean up the error
                hyper_error_free(err);

                // clean up the task
                hyper_task_free(task);

                continue;
            }

            if (hyper_task_type(task) == HYPER_TASK_EMPTY) {
                printf("internal hyper task complete\n");
                hyper_task_free(task);

                continue;
            }

            if (hyper_task_type(task) == HYPER_TASK_SERVERCONN) {
                printf("server connection task complete\n");
                hyper_task_free(task);

                continue;
            }
        }

        int timeout = hyper_executor_next_timer_pop(exec);

        printf("Processed all tasks - polling for events (max %dms)\n", timeout);

        struct epoll_event events[MAX_EVENTS];

        int nevents = epoll_wait(epoll, events, MAX_EVENTS, timeout);
        if (nevents < 0) {
            perror("epoll");
            return 1;
        }

        printf("Poll reported %d events\n", nevents);

        for (int n = 0; n < nevents; n++) {
            if (events[n].data.ptr == &listen_fd) {
                // Incoming connection(s) on listen_fd
                int new_fd;
                struct sockaddr_storage remote_addr_storage;
                struct sockaddr *remote_addr = (struct sockaddr *)&remote_addr_storage;
                socklen_t remote_addr_len = sizeof(struct sockaddr_storage);
                while ((new_fd = accept(
                            listen_fd, (struct sockaddr *)&remote_addr_storage, &remote_addr_len
                        )) >= 0) {
                    service_userdata *userdata = create_service_userdata();
                    if (getnameinfo(
                            remote_addr,
                            remote_addr_len,
                            userdata->host,
                            sizeof(userdata->host),
                            userdata->port,
                            sizeof(userdata->port),
                            NI_NUMERICHOST | NI_NUMERICSERV
                        ) < 0) {
                        perror("getnameinfo");
                        printf("New incoming connection from (unknown)\n");
                    } else {
                        printf("New incoming connection from (%s:%s)\n", userdata->host, userdata->port);
                    }

                    // Set non-blocking
                    if (fcntl(new_fd, F_SETFL, O_NONBLOCK) != 0) {
                        perror("fcntl(O_NONBLOCK) (transport)\n");
                        return 1;
                    }

                    // Close handle on exec(ve)
                    if (fcntl(new_fd, F_SETFD, FD_CLOEXEC) != 0) {
                        perror("fcntl(FD_CLOEXEC) (transport)\n");
                        return 1;
                    }

                    // Wire up IO
                    conn_data *conn = create_conn_data(epoll, new_fd);
                    hyper_io *io = create_io(conn);

                    // Ask hyper to drive this connection
                    hyper_service *service = hyper_service_new(server_callback);
                    hyper_service_set_userdata(service, userdata, free_service_userdata);
                    hyper_task *serverconn =
                        hyper_serve_httpX_connection(http1_opts, http2_opts, io, service);
                    hyper_executor_push(exec, serverconn);
                }

                if (errno != EAGAIN) {
                    perror("accept");
                }
            } else if (events[n].data.ptr == &signal_fd) {
                struct signalfd_siginfo siginfo;
                if (read(signal_fd, &siginfo, sizeof(struct signalfd_siginfo)) !=
                    sizeof(struct signalfd_siginfo)) {
                    perror("read (signal_fd)");
                    return 1;
                }

                if (siginfo.ssi_signo == SIGINT) {
                    printf("Caught SIGINT... exiting\n");
                    goto EXIT;
                } else if (siginfo.ssi_signo == SIGTERM) {
                    printf("Caught SIGTERM... exiting\n");
                    goto EXIT;
                } else if (siginfo.ssi_signo == SIGQUIT) {
                    printf("Caught SIGQUIT... exiting\n");
                    goto EXIT;
                } else {
                    printf("Caught unexpected signal %d... ignoring\n", siginfo.ssi_signo);
                }
            } else {
                // Existing transport socket, poke the wakers or close the socket
                conn_data *conn = events[n].data.ptr;
                if (events[n].events & EPOLLIN) {
                  if (conn->read_waker) {
                    hyper_waker_wake(conn->read_waker);
                    conn->read_waker = NULL;
                  } else {
                    conn->event_mask &= ~EPOLLIN;
                    if (!update_conn_data_registrations(conn, false)) {
                      epoll_ctl(conn->epoll_fd, EPOLL_CTL_DEL, conn->fd, NULL);
                    }
                  }
                }
                if (events[n].events & EPOLLOUT) {
                  if (conn->read_waker) {
                    hyper_waker_wake(conn->read_waker);
                    conn->read_waker = NULL;
                  } else {
                    conn->event_mask &= ~EPOLLOUT;
                    if (!update_conn_data_registrations(conn, false)) {
                      epoll_ctl(conn->epoll_fd, EPOLL_CTL_DEL, conn->fd, NULL);
                    }
                  }
                }
            }
        }
    }

EXIT:
    hyper_http1_serverconn_options_free(http1_opts);
    hyper_http2_serverconn_options_free(http2_opts);
    hyper_executor_free(exec);

    return 1;
}
