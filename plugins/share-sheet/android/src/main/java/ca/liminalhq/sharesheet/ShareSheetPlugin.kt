// Shows the Android share sheet for a file or text through Intent.createChooser
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

package ca.liminalhq.sharesheet

import android.annotation.SuppressLint
import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.ClipData
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.activity.result.ActivityResult
import androidx.core.content.FileProvider
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.IOException
import java.util.UUID

private const val TAG = "ShareSheetPlugin"

private const val STATUS_SHARED = "shared"
private const val STATUS_UNKNOWN = "unknown"

// The error codes the Rust side maps onto `ErrorCode`.
private const val CODE_INVALID_ARGUMENT = "invalid-argument"
private const val CODE_INVALID_PATH = "invalid-path"
private const val CODE_NOT_FOUND = "not-found"
private const val CODE_FAILED = "failed"

// The chosen-target broadcast and the chooser's activity result race; once the chooser has
// returned, wait this long for the broadcast before settling on `unknown`.
private const val CHOSEN_GRACE_MS = 500L

@InvokeArg
internal class ShareFileArgs {
  var path: String? = null
  var mime: String? = null
  var title: String? = null
}

@InvokeArg
internal class ShareTextArgs {
  var text: String? = null
  var title: String? = null
}

/** A provider class of the plugin's own, so it never clashes with a FileProvider the app declares. */
class ShareSheetFileProvider : FileProvider()

@TauriPlugin
class ShareSheetPlugin(private val activity: Activity) : Plugin(activity) {
  private val main = Handler(Looper.getMainLooper())

  /** The share whose chooser is open. Touched only on the main thread. */
  private var pending: PendingShare? = null

  /** One open chooser, settled exactly once with `shared` or `unknown`. */
  private inner class PendingShare(val invoke: Invoke) {
    var receiver: BroadcastReceiver? = null
    private var settled = false

    fun settle(status: String) {
      if (settled) return
      settled = true
      val result = JSObject()
      result.put("status", status)
      invoke.resolve(result)
    }

    fun fail(message: String) {
      if (settled) return
      settled = true
      invoke.reject(message, CODE_FAILED)
    }

    fun release() {
      val registered = receiver ?: return
      receiver = null
      try {
        activity.unregisterReceiver(registered)
      } catch (e: IllegalArgumentException) {
        Log.w(TAG, "the chosen-target receiver was already gone", e)
      }
    }
  }

  private fun authority(): String = "${activity.packageName}.share-sheet.fileprovider"

  /** The directories the plugin's FileProvider exposes; see res/xml/share_sheet_file_paths.xml. */
  private fun appRoots(): List<File> = listOf(activity.cacheDir, activity.filesDir)

  private fun isInsideAppStorage(file: File): Boolean =
    appRoots().any { dir ->
      val root = try {
        dir.canonicalPath
      } catch (e: IOException) {
        return@any false
      }
      file.path == root || file.path.startsWith(root + File.separator)
    }

  /** Where the Rust side may look for files: the same directories the FileProvider exposes. */
  @Command
  fun appDirs(invoke: Invoke) {
    val result = JSObject()
    result.put("cacheDir", activity.cacheDir.absolutePath)
    result.put("filesDir", activity.filesDir.absolutePath)
    invoke.resolve(result)
  }

  @Command
  fun shareFile(invoke: Invoke) {
    val args = invoke.parseArgs(ShareFileArgs::class.java)
    val path = args.path
    val mime = args.mime
    if (path.isNullOrBlank()) {
      invoke.reject("the path is empty", CODE_INVALID_PATH)
      return
    }
    if (mime.isNullOrBlank()) {
      invoke.reject("the MIME type is empty", CODE_INVALID_ARGUMENT)
      return
    }

    // The Rust side has already checked the path; this repeats the check against the canonical
    // path, so a file outside the app's own storage is never offered whatever the caller sends.
    val file = try {
      File(path).canonicalFile
    } catch (e: IOException) {
      invoke.reject("cannot resolve $path: ${e.message}", CODE_INVALID_PATH)
      return
    }
    if (!isInsideAppStorage(file)) {
      invoke.reject("$path is outside the app's cache and files directories", CODE_INVALID_PATH)
      return
    }
    if (!file.exists()) {
      invoke.reject("no file exists at $path", CODE_NOT_FOUND)
      return
    }
    if (!file.isFile) {
      invoke.reject("$path is not a regular file", CODE_INVALID_PATH)
      return
    }

    val uri = try {
      FileProvider.getUriForFile(activity, authority(), file)
    } catch (e: IllegalArgumentException) {
      invoke.reject("the FileProvider does not cover $path: ${e.message}", CODE_INVALID_PATH)
      return
    }

    val title = args.title?.takeIf { it.isNotBlank() }
    val send = Intent(Intent.ACTION_SEND).apply {
      type = mime
      putExtra(Intent.EXTRA_STREAM, uri)
      // ClipData carries the read grant through the chooser to the target, and lets the share
      // sheet show a preview.
      clipData = ClipData.newUri(activity.contentResolver, title ?: file.name, uri)
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      if (title != null) {
        putExtra(Intent.EXTRA_TITLE, title)
        putExtra(Intent.EXTRA_SUBJECT, title)
      }
    }
    present(invoke, send, title)
  }

  @Command
  fun shareText(invoke: Invoke) {
    val args = invoke.parseArgs(ShareTextArgs::class.java)
    val text = args.text
    if (text.isNullOrBlank()) {
      invoke.reject("the text is empty", CODE_INVALID_ARGUMENT)
      return
    }
    val title = args.title?.takeIf { it.isNotBlank() }
    val send = Intent(Intent.ACTION_SEND).apply {
      type = "text/plain"
      putExtra(Intent.EXTRA_TEXT, text)
      if (title != null) {
        putExtra(Intent.EXTRA_TITLE, title)
        putExtra(Intent.EXTRA_SUBJECT, title)
      }
    }
    present(invoke, send, title)
  }

  /**
   * Opens the chooser. The chooser reports the target the person picks through a broadcast,
   * which settles the share as `shared`. When the chooser returns without that broadcast (the
   * person backed out, or used a share-sheet action such as Copy that names no target), the
   * share settles as `unknown`: Android does not say which of those happened, so `cancelled` is
   * never reported here.
   */
  private fun present(invoke: Invoke, send: Intent, title: String?) {
    main.post {
      // A second share while a chooser is still open supersedes the first.
      pending?.let { previous ->
        previous.release()
        previous.settle(STATUS_UNKNOWN)
      }

      val share = PendingShare(invoke)
      val action = "${activity.packageName}.share-sheet.CHOSEN.${UUID.randomUUID()}"
      val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
          val chosen = chosenComponent(intent)
          Log.d(TAG, "the person picked $chosen")
          share.release()
          if (pending === share) pending = null
          share.settle(STATUS_SHARED)
        }
      }

      try {
        registerChosenReceiver(receiver, IntentFilter(action))
        share.receiver = receiver
        pending = share

        // Explicit to this package, as Android 14 requires of a mutable PendingIntent. It must
        // be mutable so the chooser can add EXTRA_CHOSEN_COMPONENT.
        val callback = Intent(action).setPackage(activity.packageName)
        var flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_ONE_SHOT
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
          flags = flags or PendingIntent.FLAG_MUTABLE
        }
        val sender = PendingIntent.getBroadcast(activity, action.hashCode(), callback, flags).intentSender
        val chooser = Intent.createChooser(send, title, sender)
        startActivityForResult(invoke, chooser, "onChooserClosed")
      } catch (e: Exception) {
        Log.e(TAG, "cannot show the share sheet", e)
        share.release()
        if (pending === share) pending = null
        share.fail("cannot show the share sheet: ${e.message ?: e.toString()}")
      }
    }
  }

  @ActivityCallback
  private fun onChooserClosed(invoke: Invoke, result: ActivityResult) {
    val share = pending?.takeIf { it.invoke === invoke } ?: return
    Log.d(TAG, "the chooser returned with result code ${result.resultCode}")
    main.postDelayed({
      share.release()
      if (pending === share) pending = null
      share.settle(STATUS_UNKNOWN)
    }, CHOSEN_GRACE_MS)
  }

  private fun chosenComponent(intent: Intent): ComponentName? =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      intent.getParcelableExtra(Intent.EXTRA_CHOSEN_COMPONENT, ComponentName::class.java)
    } else {
      @Suppress("DEPRECATION")
      intent.getParcelableExtra(Intent.EXTRA_CHOSEN_COMPONENT)
    }

  // Android 13 added the exported flag and Android 14 requires it; earlier releases have no flag,
  // and the receiver's action carries a random UUID, so nothing else can address it.
  @SuppressLint("UnspecifiedRegisterReceiverFlag")
  private fun registerChosenReceiver(receiver: BroadcastReceiver, filter: IntentFilter) {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      activity.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
    } else {
      activity.registerReceiver(receiver, filter)
    }
  }
}
