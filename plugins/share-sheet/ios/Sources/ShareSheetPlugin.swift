// Presents UIActivityViewController for a file or text and reports how it ended
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import LinkPresentation
import SwiftRs
import Tauri
import UIKit
import WebKit

class ShareFileArgs: Decodable {
  let path: String
  let mime: String
  let title: String?
}

class ShareTextArgs: Decodable {
  let text: String
  let title: String?
}

struct ShareResult: Encodable {
  let status: String
}

/// One share item that also supplies a subject (used by Mail and similar targets) and the
/// title shown in the share sheet's header.
final class TitledItem: NSObject, UIActivityItemSource {
  private let item: Any
  private let title: String?
  private let url: URL?

  init(item: Any, title: String?, url: URL?) {
    self.item = item
    self.title = title
    self.url = url
  }

  func activityViewControllerPlaceholderItem(_ activityViewController: UIActivityViewController)
    -> Any
  {
    item
  }

  func activityViewController(
    _ activityViewController: UIActivityViewController,
    itemForActivityType activityType: UIActivity.ActivityType?
  ) -> Any? {
    item
  }

  func activityViewController(
    _ activityViewController: UIActivityViewController,
    subjectForActivityType activityType: UIActivity.ActivityType?
  ) -> String {
    title ?? ""
  }

  @available(iOS 13.0, *)
  func activityViewControllerLinkMetadata(_ activityViewController: UIActivityViewController)
    -> LPLinkMetadata?
  {
    guard title != nil || url != nil else { return nil }
    let metadata = LPLinkMetadata()
    metadata.title = title ?? url?.lastPathComponent
    if let url = url {
      metadata.originalURL = url
    }
    return metadata
  }
}

class ShareSheetPlugin: Plugin {
  @objc public func shareFile(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(ShareFileArgs.self)
    // The Rust side has already resolved the path; check again here so the controller is never
    // handed a URL that does not name a regular file.
    let url = URL(fileURLWithPath: args.path).resolvingSymlinksInPath()
    var isDirectory: ObjCBool = false
    guard FileManager.default.fileExists(atPath: url.path, isDirectory: &isDirectory) else {
      invoke.reject("no file exists at \(args.path)", code: "not-found")
      return
    }
    if isDirectory.boolValue {
      invoke.reject("\(args.path) is not a regular file", code: "invalid-path")
      return
    }
    let title = args.title.flatMap { $0.isEmpty ? nil : $0 }
    present(invoke, item: TitledItem(item: url, title: title, url: url))
  }

  @objc public func shareText(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(ShareTextArgs.self)
    if args.text.isEmpty {
      invoke.reject("the text is empty", code: "invalid-argument")
      return
    }
    let title = args.title.flatMap { $0.isEmpty ? nil : $0 }
    present(invoke, item: TitledItem(item: args.text, title: title, url: nil))
  }

  /// Presents the share sheet from the key window's top view controller. `completed` in the
  /// completion handler is true once the chosen activity finished and false when the person
  /// dismissed the sheet, so iOS reports `shared` or `cancelled`, never `unknown`.
  private func present(_ invoke: Invoke, item: TitledItem) {
    DispatchQueue.main.async {
      guard let presenter = self.topViewController() else {
        invoke.reject("there is no view controller to present the share sheet from", code: "failed")
        return
      }

      let controller = UIActivityViewController(activityItems: [item], applicationActivities: nil)
      var settled = false
      controller.completionWithItemsHandler = { _, completed, _, error in
        // The handler can run more than once (an activity that finishes while the sheet stays
        // up, then the sheet closing); only the first answer counts.
        if settled { return }
        settled = true
        if let error = error {
          invoke.reject("the share failed: \(error.localizedDescription)", code: "failed")
          return
        }
        invoke.resolve(ShareResult(status: completed ? "shared" : "cancelled"))
      }

      // On iPad the controller is a popover and needs an anchor, or UIKit raises an exception.
      if let popover = controller.popoverPresentationController {
        let view: UIView = presenter.view
        popover.sourceView = view
        popover.sourceRect = CGRect(x: view.bounds.midX, y: view.bounds.midY, width: 0, height: 0)
        popover.permittedArrowDirections = []
      }

      presenter.present(controller, animated: true)
    }
  }

  private func topViewController() -> UIViewController? {
    let keyWindow = UIApplication.shared.connectedScenes
      .compactMap { $0 as? UIWindowScene }
      .flatMap { $0.windows }
      .first { $0.isKeyWindow }
    var top = keyWindow?.rootViewController ?? manager.viewController
    while let presented = top?.presentedViewController, !presented.isBeingDismissed {
      top = presented
    }
    return top
  }
}

@_cdecl("init_plugin_share_sheet")
func initPlugin() -> Plugin {
  return ShareSheetPlugin()
}
