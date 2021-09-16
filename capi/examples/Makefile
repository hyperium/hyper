#
# Build the example client
#

TARGET = client
TARGET2 = upload

OBJS = client.o
OBJS2 = upload.o

RPATH=$(PWD)/../../target/debug
CFLAGS = -I../include
LDFLAGS = -L$(RPATH) -Wl,-rpath,$(RPATH)
LIBS = -lhyper

all: $(TARGET) $(TARGET2)

$(TARGET): $(OBJS)
	$(CC) -o $(TARGET) $(OBJS) $(LDFLAGS) $(LIBS)

$(TARGET2): $(OBJS2)
	$(CC) -o $(TARGET2) $(OBJS2) $(LDFLAGS) $(LIBS)

clean:
	rm -f $(OBJS) $(TARGET) $(OBJS2) $(TARGET2)
