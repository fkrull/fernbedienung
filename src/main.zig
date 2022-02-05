const std = @import("std");

const evdev = @cImport({
    @cInclude("libevdev/libevdev.h");
});

const fcntl = @cImport({
    @cInclude("fcntl.h");
});

const cstring = @cImport({
    @cInclude("string.h");
});

const Error = error{
    FailedToOpenDeviceFile,
};

const EvdevDevice = struct {
    fd: c_int,
    dev: *evdev.libevdev,
    name: [*:0]const u8,

    fn open(path: [*:0]const u8) Error!EvdevDevice {
        const fd = std.c.open(path, fcntl.O_RDONLY);
        if (fd < 0) {
            return Error.FailedToOpenDeviceFile;
        }
        errdefer _ = std.c.close(fd);
        var dev: ?*evdev.libevdev = null;
        const ret = evdev.libevdev_new_from_fd(fd, &dev);
        if (ret != 0) {
            return Error.FailedToOpenDeviceFile;
        }
        errdefer evdev.libevdev_free(dev);
        const name = evdev.libevdev_get_name(dev);
        return EvdevDevice{ .fd = fd, .dev = dev orelse return Error.FailedToOpenDeviceFile, .name = name };
    }
};

pub fn main() anyerror!void {
    const dev = try EvdevDevice.open("/dev/input/event2");

    std.log.info("name: {s}", .{dev.name});
}

test "basic test" {
    try std.testing.expectEqual(10, 3 + 7);
}
