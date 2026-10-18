//! macOS notifications for agents that need you while Saturn is in the background. macOS only
//! delivers them to a signed app bundle, so a build run from source posts nothing.

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::{NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest, UNNotificationSound,
    UNUserNotificationCenter,
};

/// Whether Saturn runs as Saturn.app; outside a bundle the notification center refuses to start.
fn available() -> bool {
    NSBundle::mainBundle().bundleIdentifier().is_some()
}

/// Asks once for permission; macOS remembers the answer.
pub fn request_permission() {
    if !available() {
        return;
    }
    let done = RcBlock::new(|_granted: Bool, _error: *mut NSError| {});
    UNUserNotificationCenter::currentNotificationCenter().requestAuthorizationWithOptions_completionHandler(
        UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
        &done,
    );
}

/// Shows a notification. One `id` (one tab) keeps one notification: a newer one replaces it.
pub fn post(id: &str, title: &str, body: &str, sound: bool) {
    if !available() {
        return;
    }
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setThreadIdentifier(&NSString::from_str("saturn-agents"));
    if sound {
        content.setSound(Some(&UNNotificationSound::defaultSound()));
    }
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&NSString::from_str(id), &content, None);
    UNUserNotificationCenter::currentNotificationCenter().addNotificationRequest_withCompletionHandler(&request, None);
}
