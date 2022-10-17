#
# Build the example client
#

RPATH=$(PWD)/../../target/debug
CFLAGS = -I../include -ggdb3
LDFLAGS = -L$(RPATH) -Wl,-rpath,$(RPATH)
LIBS = -lhyper

all: client upload server

client: client.o
	$(CC) -o $@ $^ $(LDFLAGS) $(LIBS)
upload: upload.o
	$(CC) -o $@ $^ $(LDFLAGS) $(LIBS)
server: server.o
	$(CC) -o $@ $^ $(LDFLAGS) $(LIBS)

clean:
	rm -f *.o client server upload
