#
# Build the example client
#

TARGET = client

OBJS = client.o

RPATH=$(PWD)/../../target/debug
CFLAGS = -I../include
LDFLAGS = -L$(RPATH) -Wl,-rpath,$(RPATH)
LIBS = -lhyper

$(TARGET): $(OBJS)
	$(CC) -o $(TARGET) $(OBJS) $(LDFLAGS) $(LIBS)

upload: upload.o
	$(CC) -o upload upload.o $(LDFLAGS) $(LIBS)

clean:
	rm -f $(OBJS) $(TARGET)
	rm -f upload upload.o
