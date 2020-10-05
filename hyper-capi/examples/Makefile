#
# Build the example client
#

TARGET = client

OBJS = client.o

RPATH=$(PWD)/../target/debug
CFLAGS = -I../include
LDFLAGS = -L$(RPATH) -Wl,-rpath,$(RPATH)
LIBS = -lhyper_c

$(TARGET): $(OBJS)
	$(CC) -o $(TARGET) $(OBJS) $(LDFLAGS) $(LIBS)

clean:
	rm -f $(OBJS) $(TARGET)
