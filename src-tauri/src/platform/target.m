#import <AppKit/AppKit.h>
#import <ScreenCaptureKit/ScreenCaptureKit.h>
#include <stdatomic.h>

// Only the clicked area's geometry crosses the bridge; no other app's controls are queried.
typedef struct { double x, y, width, height; } MBRect;
static atomic_bool captureBusy = false;
uint32_t mb_window_id(void *window) { return (uint32_t)[(__bridge NSWindow *)window windowNumber]; }
bool mb_screen_permission(bool request) {
    if (@available(macOS 10.15, *)) return request ? CGRequestScreenCaptureAccess() : CGPreflightScreenCaptureAccess();
    return false;
}
// One outstanding capture, including late callbacks after timeout. No frames are written to disk.
unsigned char *mb_capture(MBRect rect, double scale, uint32_t excluded, size_t *length) {
    *length = 0;
    if (@available(macOS 14.0, *)) {
    if (!CGPreflightScreenCaptureAccess() || atomic_exchange(&captureBusy, true)) return NULL;
    @autoreleasepool {
        dispatch_semaphore_t done = dispatch_semaphore_create(0);
        __block NSData *png = nil;
        [SCShareableContent getShareableContentExcludingDesktopWindows:YES onScreenWindowsOnly:YES completionHandler:^(SCShareableContent *content, NSError *error) {
            SCDisplay *display = nil;
            for (SCDisplay *candidate in content.displays) if (candidate.displayID == CGMainDisplayID()) { display = candidate; break; }
            if (!display || error) { atomic_store(&captureBusy,false); dispatch_semaphore_signal(done); return; }
            NSMutableArray *excludedWindows = [NSMutableArray array];
            for (SCWindow *window in content.windows) if (window.windowID == excluded) [excludedWindows addObject:window];
            SCContentFilter *filter = [[SCContentFilter alloc] initWithDisplay:display excludingWindows:excludedWindows];
            SCStreamConfiguration *config = [SCStreamConfiguration new];
            config.sourceRect = CGRectMake(rect.x-display.frame.origin.x,rect.y-display.frame.origin.y,rect.width,rect.height);
            config.width = (size_t)ceil(rect.width * scale); config.height = (size_t)ceil(rect.height * scale);
            config.showsCursor = NO;
            if (@available(macOS 14.0, *)) {
                [SCScreenshotManager captureImageWithFilter:filter configuration:config completionHandler:^(CGImageRef image, NSError *failure) {
                    if (image && !failure) png = [[[NSBitmapImageRep alloc] initWithCGImage:image] representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
                    atomic_store(&captureBusy,false); dispatch_semaphore_signal(done);
                }];
            } else { atomic_store(&captureBusy,false); dispatch_semaphore_signal(done); }
        }];
        if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 650 * NSEC_PER_MSEC)) != 0) return NULL;
        if (!png || png.length > 8 * 1024 * 1024) return NULL;
        unsigned char *bytes = malloc(png.length);
        if (bytes) { memcpy(bytes,png.bytes,png.length); *length = png.length; }
        return bytes;
    }
    } else { return NULL; }
}
void mb_free(void *bytes) { free(bytes); }
