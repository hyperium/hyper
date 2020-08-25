#ifndef _HYPER_H
#define _HYPER_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stddef.h>

#define TODO void

typedef struct hyper_clientconn hyper_clientconn;

typedef struct hyper_clientconn_options hyper_clientconn_options;

typedef struct hyper_io hyper_io;

typedef struct hyper_request hyper_request;

typedef struct hyper_response hyper_response;

typedef struct hyper_headers hyper_headers;

typedef struct hyper_task hyper_task;

typedef struct hyper_waker hyper_waker;

typedef struct hyper_executor hyper_executor;

typedef struct hyper_str {
	const uint8_t *buf;
	size_t len;
} hyper_str;

typedef enum {
    HYPERE_OK,
    HYPERE_KABOOM,
} hyper_error;

typedef enum {
	HYPER_IT_CONTINUE,
	HYPER_IT_BREAK,
} hyper_iter_step;

// HTTP ClientConn

// Starts an HTTP client connection handshake using the provided IO transport
// and options.
//
// Both the `io` and the `options` are consumed in this function call.
//
// The returned `hyper_task *` must be polled with an executor until the
// handshake completes, at which point the value can be taken.
hyper_task *hyper_clientconn_handshake(hyper_io *io, hyper_clientconn_options *options);

// Send a request on the client connection.
//
// Returns a task that needs to be polled until it is ready. When ready, the
// task yields a `hyper_response *`.
hyper_task *hyper_clientconn_send(hyper_clientconn *client, hyper_request *request);

// Creates a new set of HTTP clientconn options to be used in a handshake.
hyper_clientconn_options *hyper_clientconn_options_new();

// Frees options not passed to a handshake.
void hyper_clientconn_options_free(hyper_clientconn_options *options);

// HTTP IO

// Create a new IO type used to represent a transport.
//
// The read and write functions of this transport should be set with
// `hyper_io_set_read` and `hyper_io_set_write`.
hyper_io *hyper_io_new();

// Set the user data pointer for this IO to some value.
//
// This value is passed as an argument to the read and write callbacks.
void hyper_io_set_data(hyper_io *io, void *userdata);

#define HYPER_IO_PENDING 0xFFFFFFFF
#define HYPER_IO_ERROR 0xFFFFFFFE

// Set the read function for this IO transport.
//
// Data that is read from the transport should be put in the `buf` pointer,
// up to `buf_len` bytes. The number of bytes read should be the return value.
//
// If there is no data currently available, the `waker` should be cloned and
// registered with whatever polling mechanism is used to signal when data
// is available later on. The return value should be `HYPER_IO_PENDING`.
//
// If there is an irrecoverable error reading data, then `HYPER_IO_ERROR`
// should be the return value.
void hyper_io_set_read(hyper_io *io, size_t (*func)(void *userdata, hyper_waker *waker, uint8_t *buf, size_t buf_len));

// Set the write function for this IO transport.
//
// Data from the `buf` pointer should be written to the transport, up to
// `buf_len` bytes. The number of bytes written should be the return value.
//
// If no data can currently be written, the `waker` should be cloned and
// registered with whatever polling mechanism is used to signal when data
// is available later on. The return value should be `HYPER_IO_PENDING`.
//
// If there is an irrecoverable error reading data, then `HYPER_IO_ERROR`
// should be the return value.
void hyper_io_set_write(hyper_io *io, size_t (*func)(void *userdata, hyper_waker *waker, const uint8_t *buf, size_t buf_len));

// HTTP Requests

// Construct a new HTTP request.
hyper_request *hyper_request_new();

// Free an HTTP request if not going to send it on a client.
void hyper_request_free(hyper_request *request);

// Set the HTTP Method of the request.
hyper_error hyper_request_set_method(hyper_request *request, uint8_t *method, size_t method_len);

// Set the URI of the request.
hyper_error hyper_request_set_uri(hyper_request *request, uint8_t *uri, size_t uri_len);

hyper_error hyper_request_add_header(hyper_request *request, uint8_t *name, size_t name_len, uint8_t *value, size_t value_len);


// HTTP Responses

// Free an HTTP response after using it.
void hyper_response_free(hyper_response *response);

// Get the HTTP-Status code of this response.
//
// It will always be within the range of 100-599.
uint16_t hyper_response_status(hyper_response *response);

// Gets a reference to the HTTP headers of this response.
//
// This is not an owned reference, so it should not be accessed after the
// `hyper_response` has been freed.
hyper_headers *hyper_response_headers(hyper_response *response);

// HTTP Headers

// Sets the header with the provided name to the provided value.
//
// This overwrites any previous value set for the header.
void hyper_headers_set(hyper_headers *headers, hyper_str name, hyper_str value);

// Adds the provided value to the list of the provided name.
//
// If there were already existing values for the name, this will append the
// new value to the internal list.
void hyper_headers_add(hyper_headers *headers, hyper_str name, hyper_str value);

// Iterates the headers passing each name and value pair to the callback.
//
// The `userdata` pointer is also passed to the callback.
//
// The callback should return `HYPER_IT_CONTINUE` to keep iterating, or
// `HYPER_IT_BREAK` to stop.
void hyper_headers_iter(hyper_headers *headers,
                        hyper_iter_step (*func)(void *userdata,
                                                hyper_str name,
                                                hyper_str value),
                        void *userdata);

// Futures and Executors

hyper_executor *hyper_executor_new();

// Push a task onto the executor.
void hyper_executor_push(hyper_executor *executor, hyper_task *task);

// Polls the executor, trying to make progress on any tasks that have notified
// that they are ready again.
void hyper_executor_poll(hyper_executor *executor);

// Pop a task from the executor that has completed.
//
// If there are no ready tasks, this returns `NULL`.
hyper_task *hyper_executor_pop(hyper_executor *executor);

// Frees an executor, and any tasks it may currently be holding.
void hyper_executor_free(hyper_executor *executor);

typedef enum {
	HYPER_TASK_BG,
	HYPER_TASK_CLIENTCONN_HANDSHAKE,
	HYPER_TASK_CLIENT_SEND,
} hyper_task_return_type;

// Query the return type of this task.
hyper_task_return_type hyper_task_type(hyper_task *task);

// Returns `1` if this task concluded with an error, `0` otherwise.
int hyper_task_is_error(hyper_task *task);

// Takes the output value of this task.
//
// This must only be called once polling the task on an executor has finished
// this task.
//
// Use `hyper_task_type` to determine the type of the `void *` return value.
void *hyper_task_value(hyper_task *task);

// Free a task.
void hyper_task_free(hyper_task *task);

// Clone a reference to this waker.
hyper_waker *hyper_waker_clone(hyper_waker *waker);

// Free the reference to a waker.
void hyper_waker_free(hyper_waker *waker);

#ifdef __cplusplus
}
#endif

#endif
