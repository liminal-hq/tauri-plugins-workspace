---
'desktop-integration': minor
'desktop-integration-js': minor
'xdg-portal': minor
'xdg-portal-js': minor
---

Notifications can carry action buttons. `sendNotification` in `xdg-portal` and `notify` in `desktop-integration` take an optional `actions` list of `{ id, label }` (at most 3; extras are dropped and logged; ids and labels are validated). `xdg-portal` maps them to the Notification portal's `buttons`, and `desktop-integration` to the `actions` array of `org.freedesktop.Notifications.Notify` after the `default` key. A pressed button, like the default click, arrives through the existing `notification-action` event as `{ id, action }` with the notification's own `id` and the button's id as `action`; `desktop-integration` ignores signals for notifications it did not send. `get_status` gains a `notificationActions` feature: available when the Notification portal is version 1 or later (`xdg-portal`) or the server lists the `actions` capability (`desktop-integration`), otherwise unavailable with the new `actions-unsupported` reason and a `detail` explaining that only the default click is offered. Windows toasts do not show buttons yet and report the feature as unavailable.
