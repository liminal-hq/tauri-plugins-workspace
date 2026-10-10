// Android haptics plugin: parses requests, plays them on the vibrator and reports capabilities
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

package ca.liminalhq.haptics

import android.app.Activity
import android.content.Context
import android.media.AudioAttributes
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.os.VibrationAttributes
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.provider.Settings
import android.view.HapticFeedbackConstants
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import org.json.JSONArray
import org.json.JSONObject

@InvokeArg
internal class AndroidConfigArgs {
  var foregroundAudioUsage: String? = null
  var backgroundAudioUsage: String? = null
}

@InvokeArg
internal class PluginConfigArgs {
  var defaultUsage: String? = null
  var respectSystemHapticsSetting: Boolean? = null
  var stopBeforePlay: Boolean? = null
  var maxDurationMs: Long? = null
  var maxAmplitude: Int? = null
  var allowRepeatingWaveforms: Boolean? = null
  var android: AndroidConfigArgs? = null
}

@InvokeArg
internal class EffectRequestArgs {
  // NOTE: union types are easiest to parse as a JSObject
  var id: String? = null
  var usage: String? = null
  var respectSystemSettings: Boolean? = null
  var stopBeforePlay: Boolean? = null
  lateinit var effect: JSObject
}

private val USAGES = setOf("touch", "notification", "alarm", "media")

private val PRIMITIVE_IDS = listOf("tick", "low_tick", "click", "thud", "spin", "quick_rise", "slow_rise")

private val EFFECT_IDS = listOf("click", "double_click", "tick", "heavy_click")

/** A key that is set to a real value; Tauri's `has` is also true for an explicit `null`. */
private fun JSObject.present(key: String): Boolean = has(key) && !isNull(key)

// Built-in durations used when the motor does not report its own (milliseconds).
private val PRIMITIVE_MS = mapOf(
  "tick" to 10, "low_tick" to 12, "click" to 15, "thud" to 30,
  "quick_rise" to 60, "slow_rise" to 150, "spin" to 90,
)

// Rough length of a system UI tick, for the result estimate.
private const val UI_FEEDBACK_MS = 20L

// The most steps one `play_steps` call may schedule.
private const val MAX_STEPS = 512

private val PREDEFINED_MS = mapOf("click" to 15L, "double_click" to 60L, "tick" to 10L, "heavy_click" to 30L)

// Nearest supported stand-in, tried in order, when a motor lacks a primitive.
private val PRIMITIVE_NEIGHBOURS = mapOf(
  "low_tick" to listOf("tick", "click"),
  "tick" to listOf("click"),
  "thud" to listOf("click"),
  "spin" to listOf("quick_rise"),
  "slow_rise" to listOf("quick_rise"),
)

@TauriPlugin
class HapticsPlugin(private val activity: Activity) : Plugin(activity) {

  private var cfg = PluginConfigArgs()
  private val vibrator: Vibrator by lazy { getVibrator(activity) }

  private var webView: WebView? = null
  private val stepHandler = Handler(Looper.getMainLooper())
  private val stepToken = Any()
  @Volatile private var stepGeneration = 0

  override fun load(webView: WebView) {
    this.webView = webView
    // Pull config from tauri.conf.json if present
    runCatching { getConfig(PluginConfigArgs::class.java) }.onSuccess {
      cfg = it
    }
  }

  @Command
  fun capabilities(invoke: Invoke) {
    val ret = JSObject()
    val sdk = Build.VERSION.SDK_INT

    val hasVibrator = vibrator.hasVibrator()
    val hasAmplitude = if (hasVibrator) vibrator.hasAmplitudeControl() else false
    ret.put("platform", "android")
    ret.put("sdkInt", sdk)
    ret.put("hasVibrator", hasVibrator)
    ret.put("hasAmplitudeControl", hasAmplitude)

    // Composition primitives, reported one by one: a missing primitive must not hide the rest.
    val primitives = primitiveSupport(hasVibrator)
    val prims = JSObject()
    var anyPrimitive = false
    for ((id, support) in primitives) {
      val entry = JSObject()
      entry.put("supported", support.first)
      entry.put("durationMs", support.second ?: JSONObject.NULL)
      prims.put(id, entry)
      if (support.first) anyPrimitive = true
    }
    ret.put("primitives", prims)
    ret.put("compositionSupported", sdk >= 30 && anyPrimitive)

    ret.put("effects", effectSupport(hasVibrator))

    // Envelope effects (API 36+); gated on device support
    val envelopeSupported = hasVibrator && envelopeEffectsSupported()
    ret.put("envelopeSupported", envelopeSupported)
    if (envelopeSupported) {
      envelopeInfo()?.let { ret.put("envelopeInfo", it) }
    }

    if (hasVibrator && sdk >= 31) {
      val resonant = runCatching { vibrator.resonantFrequency }.getOrDefault(Float.NaN)
      if (!resonant.isNaN()) ret.put("resonantHz", resonant.toDouble())
      val q = runCatching { vibrator.qFactor }.getOrDefault(Float.NaN)
      if (!q.isNaN()) ret.put("qFactor", q.toDouble())
    }

    ret.put("topTier", topTier(hasVibrator, envelopeSupported, anyPrimitive, hasAmplitude))

    // The touch-feedback setting; `hapticFeedbackEnabled` is the deprecated name for one release.
    val enabled = touchFeedbackEnabled()
    ret.put("touchFeedbackEnabled", enabled ?: JSONObject.NULL)
    if (enabled != null) ret.put("hapticFeedbackEnabled", enabled)

    val limits = JSObject()
    limits.put("maxDurationMs", (cfg.maxDurationMs ?: 10_000).coerceAtLeast(1))
    limits.put("maxAmplitude", (cfg.maxAmplitude ?: 255).coerceIn(1, 255))
    limits.put("allowRepeatingWaveforms", cfg.allowRepeatingWaveforms ?: false)
    ret.put("limits", limits)

    val device = JSObject()
    device.put("manufacturer", Build.MANUFACTURER)
    device.put("model", Build.MODEL)
    device.put("release", Build.VERSION.RELEASE)
    ret.put("device", device)

    invoke.resolve(ret)
  }

  private fun touchFeedbackEnabled(): Boolean? = try {
    Settings.System.getInt(activity.contentResolver, Settings.System.HAPTIC_FEEDBACK_ENABLED, 1) != 0
  } catch (_: Throwable) { null }

  private fun topTier(
    hasVibrator: Boolean,
    envelope: Boolean,
    anyPrimitive: Boolean,
    amplitude: Boolean,
  ): Int = when {
    !hasVibrator -> 0
    envelope -> 4
    anyPrimitive -> 3
    amplitude -> 2
    else -> 1
  }

  /** Per primitive id: (supported, measured duration in ms or null). */
  private fun primitiveSupport(hasVibrator: Boolean): Map<String, Pair<Boolean, Int?>> {
    val ids = PRIMITIVE_IDS
    if (!hasVibrator || Build.VERSION.SDK_INT < 30) {
      return ids.associateWith { Pair(false, null) }
    }
    val constants = ids.map { primitiveConstant(it) }
    val supported = runCatching { vibrator.arePrimitivesSupported(*constants.toIntArray()) }
      .getOrDefault(BooleanArray(ids.size))
    val durations = if (Build.VERSION.SDK_INT >= 31) {
      runCatching { vibrator.getPrimitiveDurations(*constants.toIntArray()) }.getOrNull()
    } else null

    return ids.indices.associate { i ->
      val ok = supported.getOrElse(i) { false }
      val ms = durations?.getOrNull(i)?.takeIf { ok && it > 0 }
      ids[i] to Pair(ok, ms)
    }
  }

  /** `yes`, `no` or `unknown` per predefined effect (API 30+; unknown below). */
  private fun effectSupport(hasVibrator: Boolean): JSObject {
    val out = JSObject()
    val ids = EFFECT_IDS
    val results = if (hasVibrator && Build.VERSION.SDK_INT >= 30) {
      runCatching {
        vibrator.areEffectsSupported(*ids.map { predefinedConstant(it) }.toIntArray())
      }.getOrNull()
    } else null

    for ((i, id) in ids.withIndex()) {
      val label = when (results?.getOrNull(i)) {
        Vibrator.VIBRATION_EFFECT_SUPPORT_YES -> "yes"
        Vibrator.VIBRATION_EFFECT_SUPPORT_NO -> "no"
        else -> if (hasVibrator) "unknown" else "no"
      }
      out.put(id, label)
    }
    return out
  }

  /**
   * The UI lane: system-style feedback through `View.performHapticFeedback`, which the OS tunes and
   * which follows the touch-feedback setting. Newer constants fall back on older releases.
   */
  @Command
  fun ui(invoke: Invoke) {
    val kind = invoke.getArgs().optString("kind", "")
    val choice = uiFeedback(kind)
    if (choice == null) {
      invoke.reject("Unknown UI feedback kind: $kind", "INVALID_EFFECT")
      return
    }

    if (!vibrator.hasVibrator()) {
      invoke.resolve(playResult(0, 0, listOf("No vibrator on this device")))
      return
    }
    if (touchFeedbackEnabled() == false) {
      invoke.resolve(playResult(0, 0, listOf("Touch feedback is off in system settings")))
      return
    }
    val view = webView
    if (view == null) {
      invoke.resolve(playResult(0, 0, listOf("The web view is not ready")))
      return
    }

    val tier = minOf(deviceTopTier(), 3)
    activity.runOnUiThread {
      val played = runCatching { view.performHapticFeedback(choice.first) }.getOrDefault(false)
      val reasons = mutableListOf<String>()
      if (choice.second != null) reasons.add(choice.second!!)
      if (!played) reasons.add("The system did not play the feedback")
      invoke.resolve(playResult(if (played) tier else 0, if (played) UI_FEEDBACK_MS else 0, reasons))
    }
  }

  /** (constant, fallback note) for a UI kind; null for an unknown kind. */
  private fun uiFeedback(kind: String?): Pair<Int, String?>? {
    val sdk = Build.VERSION.SDK_INT
    return when (kind) {
      "confirm" -> if (sdk >= 30) Pair(HapticFeedbackConstants.CONFIRM, null)
        else Pair(HapticFeedbackConstants.CLOCK_TICK, "CONFIRM needs API 30; fell back to CLOCK_TICK")
      "reject" -> if (sdk >= 30) Pair(HapticFeedbackConstants.REJECT, null)
        else Pair(HapticFeedbackConstants.CONTEXT_CLICK, "REJECT needs API 30; fell back to CONTEXT_CLICK")
      "tick" -> Pair(HapticFeedbackConstants.CLOCK_TICK, null)
      "toggle-on" -> if (sdk >= 34) Pair(HapticFeedbackConstants.TOGGLE_ON, null)
        else Pair(HapticFeedbackConstants.CLOCK_TICK, "TOGGLE_ON needs API 34; fell back to CLOCK_TICK")
      "toggle-off" -> if (sdk >= 34) Pair(HapticFeedbackConstants.TOGGLE_OFF, null)
        else Pair(HapticFeedbackConstants.CLOCK_TICK, "TOGGLE_OFF needs API 34; fell back to CLOCK_TICK")
      "drag-start" -> if (sdk >= 34) Pair(HapticFeedbackConstants.DRAG_START, null)
        else Pair(HapticFeedbackConstants.CLOCK_TICK, "DRAG_START needs API 34; fell back to CLOCK_TICK")
      else -> null
    }
  }

  @Command
  fun stop(invoke: Invoke) {
    cancelScheduled()
    if (vibrator.hasVibrator()) vibrator.cancel()
    invoke.resolve(JSObject())
  }

  @Command
  fun play(invoke: Invoke) {
    val argsRoot = invoke.getArgs()
    val args = argsRoot.getJSObject("req") ?: argsRoot

    val prepared = try {
      prepare(args)
    } catch (e: Throwable) {
      invoke.reject(e.message ?: "Invalid haptics request", "INVALID_EFFECT")
      return
    }

    when (prepared) {
      is Prepared.Silent -> invoke.resolve(playResult(0, 0, prepared.reasons))
      is Prepared.Ready -> {
        if (prepared.stopBefore) {
          cancelScheduled()
          vibrator.cancel()
        }
        vibrate(prepared.built.effect!!, prepared.usage)
        invoke.resolve(playResult(prepared.built.tier, prepared.built.estimatedMs, prepared.built.reasons))
      }
    }
  }

  /**
   * Plays a compiled step list. Every step is validated first, then all are scheduled on one
   * handler from a single start time so the rhythm between them stays tight. `stop` cancels it.
   */
  @Command
  fun play_steps(invoke: Invoke) {
    val steps = getArray(invoke.getArgs(), "steps")
    if (steps.length() == 0) {
      invoke.reject("steps cannot be empty", "INVALID_EFFECT")
      return
    }
    if (steps.length() > MAX_STEPS) {
      invoke.reject("steps exceeds the maximum of $MAX_STEPS", "INVALID_EFFECT")
      return
    }

    val maxDur = (cfg.maxDurationMs ?: 10_000).coerceAtLeast(1)
    val ready = mutableListOf<Pair<Long, Prepared.Ready>>()
    val reasons = linkedSetOf<String>()
    var tier = 0
    var end = 0L
    try {
      for (i in 0 until steps.length()) {
        val step = getObject(steps, i)
        if (!step.has("atMs")) throw IllegalArgumentException("steps[$i]: missing field `atMs`")
        val atMs = step.getLong("atMs")
        if (atMs < 0) throw IllegalArgumentException("steps[$i]: atMs must not be negative")
        if (atMs > maxDur) {
          throw IllegalArgumentException("steps[$i]: atMs $atMs exceeds the limit of $maxDur ms")
        }
        val request = step.getJSObject("request")
          ?: throw IllegalArgumentException("steps[$i]: missing field `request`")
        val prepared = try {
          // A step may only use what is left of the duration cap after its start offset.
          prepare(request, maxDur - atMs)
        } catch (e: IllegalArgumentException) {
          throw IllegalArgumentException("steps[$i]: ${e.message}")
        }
        when (prepared) {
          is Prepared.Silent -> reasons.addAll(prepared.reasons)
          is Prepared.Ready -> {
            ready.add(Pair(atMs, prepared))
            reasons.addAll(prepared.built.reasons)
            tier = maxOf(tier, prepared.built.tier)
            end = maxOf(end, atMs + prepared.built.estimatedMs)
          }
        }
      }
    } catch (e: Throwable) {
      invoke.reject(e.message ?: "Invalid haptics request", "INVALID_EFFECT")
      return
    }

    if (ready.isNotEmpty()) {
      cancelScheduled()
      vibrator.cancel()
      val generation = stepGeneration
      val start = SystemClock.uptimeMillis()
      for ((atMs, step) in ready) {
        stepHandler.postAtTime({
          // Superseded by a stop or a newer call, or the OS refused the effect: stay quiet.
          if (generation == stepGeneration) {
            runCatching { vibrate(step.built.effect!!, step.usage) }
          }
        }, stepToken, start + atMs)
      }
    }
    invoke.resolve(playResult(tier, end, reasons.toList()))
  }

  /**
   * Cancels pending steps. The generation moves on first, so a step runnable that is already
   * executing on the main thread sees it has been superseded and does not vibrate after the stop.
   */
  private fun cancelScheduled() {
    stepGeneration++
    stepHandler.removeCallbacksAndMessages(stepToken)
  }

  private sealed class Prepared {
    /** The request resolves without playing anything. */
    class Silent(val reasons: List<String>) : Prepared()

    class Ready(val built: Built, val usage: String, val stopBefore: Boolean) : Prepared()
  }

  /** The checks that do not depend on the hardware or on any setting. */
  private fun validateEffect(effectObj: JSObject) {
    when (val type = effectObj.getString("type")) {
      "oneshot" -> {
        if (getLong(effectObj, "durationMs", "duration_ms") <= 0) {
          throw IllegalArgumentException("durationMs must be positive")
        }
        // Absent means the default strength; an explicit value must be a real one.
        if (effectObj.present("amplitude") && effectObj.getInt("amplitude") !in 1..255) {
          throw IllegalArgumentException("amplitude must be within 1..255")
        }
      }

      "waveform" -> {
        val timings = toLongArray(getArray(effectObj, "timingsMs", "timings_ms"))
        if (timings.isEmpty()) throw IllegalArgumentException("timingsMs cannot be empty")
        if (timings.all { it == 0L }) throw IllegalArgumentException("at least one timing must be non-zero")
        if (effectObj.present("amplitudes") &&
          getArray(effectObj, "amplitudes", "amplitudes_ms").length() != timings.size
        ) {
          throw IllegalArgumentException("amplitudes must have same length as timingsMs")
        }
      }

      "predefined" -> {
        val id = getString(effectObj, "effectId", "effect_id").lowercase()
        if (id !in EFFECT_IDS) {
          throw IllegalArgumentException(
            "Unknown predefined effect `$id`. Use one of ${EFFECT_IDS.joinToString(", ")}; " +
              "for a thud use the `thud` composition primitive."
          )
        }
      }

      "composition" -> {
        val steps = getArray(effectObj, "steps")
        for (i in 0 until steps.length()) {
          val step = getObject(steps, i)
          val kind = step.getString("kind")
          if (kind != "primitive") {
            throw IllegalArgumentException(
              "steps[$i]: unsupported step kind `$kind`. Compositions are primitives only."
            )
          }
          val requested = step.getString("primitive").lowercase()
          if (requested !in PRIMITIVE_IDS) {
            throw IllegalArgumentException("steps[$i]: unknown primitive `$requested`")
          }
        }
      }

      "envelopeWaveform" -> validateEnvelopeShape(effectObj)

      else -> throw IllegalArgumentException("Unknown effect type: $type")
    }
  }

  /** Turns a request into an effect, or says why nothing will play. Throws for invalid input. */
  private fun prepare(args: JSObject, budgetMs: Long = Long.MAX_VALUE): Prepared {
    val effectObj = args.getJSObject("effect") ?: throw IllegalArgumentException("Missing effect payload")
    // Invalid input rejects on every device, including one that has nothing to vibrate.
    validateEffect(effectObj)

    val requested = (args.getString("usage", cfg.defaultUsage ?: "touch") ?: "touch").lowercase()
    val usage = if (requested in USAGES) requested else "touch"

    // The touch-feedback setting gates touch-usage haptics only, so a media or alarm rumble is not
    // muted by it. An explicit `respectSystemSettings` on the request still wins.
    val respect = if (args.present("respectSystemSettings")) {
      args.getBoolean("respectSystemSettings")
    } else {
      usage == "touch" && (cfg.respectSystemHapticsSetting ?: true)
    }
    if (respect && touchFeedbackEnabled() == false) {
      return Prepared.Silent(listOf("Touch feedback is off in system settings"))
    }

    if (!vibrator.hasVibrator()) {
      return Prepared.Silent(listOf("No vibrator on this device"))
    }

    val stopBefore = if (args.present("stopBeforePlay")) {
      args.getBoolean("stopBeforePlay")
    } else {
      cfg.stopBeforePlay ?: true
    }

    val maxAmp = (cfg.maxAmplitude ?: 255).coerceIn(1, 255)
    val maxDur = minOf((cfg.maxDurationMs ?: 10_000).coerceAtLeast(1), budgetMs).coerceAtLeast(1)

    val built = buildEffect(effectObj, vibrator, maxAmp, maxDur, cfg)
    // A request the device cannot play resolves at tier 0 with the reason, never silently.
    if (built.effect == null) return Prepared.Silent(built.reasons)
    return Prepared.Ready(built, usage, stopBefore)
  }

  /**
   * API 33+ takes `VibrationAttributes`, which carry the real usage (touch, notification, alarm,
   * media); older releases get the nearest `AudioAttributes` usage.
   */
  private fun vibrate(effect: VibrationEffect, usage: String) {
    if (Build.VERSION.SDK_INT >= 33) {
      val attrs = runCatching {
        VibrationAttributes.createForUsage(vibrationUsage(usage))
      }.getOrNull()
      if (attrs != null) {
        vibrator.vibrate(effect, attrs)
        return
      }
    }
    vibrator.vibrate(effect, audioAttributesForUsage(usage, cfg))
  }

  private fun vibrationUsage(usage: String): Int = when (usage) {
    "alarm" -> VibrationAttributes.USAGE_ALARM
    "notification" -> VibrationAttributes.USAGE_NOTIFICATION
    "media" -> VibrationAttributes.USAGE_MEDIA
    else -> VibrationAttributes.USAGE_TOUCH
  }

  /** What a request was turned into: the effect, the tier it plays at and everything that changed. */
  private class Built(
    val effect: VibrationEffect?,
    val tier: Int,
    val estimatedMs: Long,
    val reasons: List<String> = emptyList(),
  )

  private fun playResult(tier: Int, estimatedMs: Long, reasons: List<String>): JSObject {
    val ret = JSObject()
    ret.put("ok", true)
    ret.put("tier", tier)
    ret.put("target", "phone")
    ret.put("estimatedMs", estimatedMs)
    ret.put("downgraded", reasons.isNotEmpty())
    if (reasons.isNotEmpty()) {
      val reason = reasons.joinToString(" · ")
      ret.put("reason", reason)
      ret.put("downgradeReason", reason) // deprecated alias, kept for one release
    }
    return ret
  }

  private fun buildEffect(
    effectObj: JSObject,
    vibrator: Vibrator,
    maxAmp: Int,
    maxDur: Long,
    cfg: PluginConfigArgs,
  ): Built {

    val type = effectObj.getString("type")

    return when (type) {
      "oneshot" -> {
        val requestedDur = getLong(effectObj, "durationMs", "duration_ms")
        val dur = requestedDur.coerceAtMost(maxDur)
        val ampRaw = if (effectObj.present("amplitude")) effectObj.getInt("amplitude") else -1
        val hasAmplitude = vibrator.hasAmplitudeControl()
        val reasons = mutableListOf<String>()
        if (dur < requestedDur) reasons.add("Truncated to $maxDur ms")
        val amp = when {
          ampRaw <= 0 -> VibrationEffect.DEFAULT_AMPLITUDE
          !hasAmplitude -> {
            reasons.add("No amplitude control; played at default strength")
            VibrationEffect.DEFAULT_AMPLITUDE
          }
          else -> ampRaw.coerceIn(1, maxAmp)
        }
        Built(VibrationEffect.createOneShot(dur, amp), if (hasAmplitude) 2 else 1, dur, reasons)
      }

      "waveform" -> {
        val timings = toLongArray(getArray(effectObj, "timingsMs", "timings_ms"))
        val repeat = if (effectObj.present("repeat")) effectObj.getInt("repeat") else -1

        // Enforce repeat safety, and say so
        val allowRepeat = cfg.allowRepeatingWaveforms ?: false
        val safeRepeat = if (!allowRepeat && repeat >= 0) -1 else repeat
        val reasons = mutableListOf<String>()
        if (safeRepeat != repeat) reasons.add("Repeat ignored: allowRepeatingWaveforms is false")
        val capped = capWaveformDuration(timings, maxDur)
        if (timings.sum() > maxDur) reasons.add("Truncated to $maxDur ms")

        if (capped.isEmpty()) {
          throw IllegalArgumentException("timingsMs cannot be empty")
        }
        if (capped.all { it == 0L }) {
          throw IllegalArgumentException("at least one timing must be non-zero")
        }
        val total = capped.sum()
        val hasAmplitude = vibrator.hasAmplitudeControl()

        if (effectObj.present("amplitudes")) {
          val amps = toIntArray(getArray(effectObj, "amplitudes", "amplitudes_ms")).map { it.coerceIn(0, maxAmp) }.toIntArray()
          if (amps.size != capped.size) {
            throw IllegalArgumentException("amplitudes must have same length as timingsMs")
          }
          val eff = if (hasAmplitude) {
            VibrationEffect.createWaveform(capped, amps, safeRepeat)
          } else {
            // Downgrade: non-zero amplitudes become the default strength. The timings-only overload
            // would start with an off phase, so the amplitude layout is kept instead.
            reasons.add("Device lacks amplitude control")
            val onOff = IntArray(amps.size) { if (amps[it] > 0) VibrationEffect.DEFAULT_AMPLITUDE else 0 }
            VibrationEffect.createWaveform(capped, onOff, safeRepeat)
          }
          Built(eff, if (hasAmplitude) 2 else 1, total, reasons)
        } else {
          Built(VibrationEffect.createWaveform(capped, safeRepeat), 1, total, reasons)
        }
      }

      "predefined" -> {
        val id = getString(effectObj, "effectId", "effect_id").lowercase()
        if (id !in EFFECT_IDS) {
          throw IllegalArgumentException(
            "Unknown predefined effect `$id`. Use one of ${EFFECT_IDS.joinToString(", ")}; " +
              "for a thud use the `thud` composition primitive."
          )
        }
        val support = if (Build.VERSION.SDK_INT >= 30) {
          runCatching { vibrator.areEffectsSupported(predefinedConstant(id)).firstOrNull() }.getOrNull()
        } else null
        if (support == Vibrator.VIBRATION_EFFECT_SUPPORT_NO) {
          Built(null, 0, 0, listOf("This device does not support the predefined effect `$id`"))
        } else {
          val ms = PREDEFINED_MS[id] ?: 20L
          if (Build.VERSION.SDK_INT < 29) {
            boundedPredefined(
              predefinedConstant(id), ms, maxDur, 1, listOf("Predefined effects require API 29+; played a pulse"),
            )
          } else {
            boundedPredefined(predefinedConstant(id), ms, maxDur, minOf(deviceTopTier(), 3))
          }
        }
      }

      "composition" -> {
        if (Build.VERSION.SDK_INT < 30) {
          // Downgrade to a click
          boundedPredefined(
            VibrationEffect.EFFECT_CLICK, PREDEFINED_MS.getValue("click"), maxDur, 1,
            listOf("Composition requires API 30+"),
          )
        } else {
          buildComposition(effectObj, maxDur)
        }
      }

      "envelopeWaveform" -> {
        validateEnvelopeShape(effectObj)
        if (!envelopeEffectsSupported()) {
          val reason = if (Build.VERSION.SDK_INT < 36) {
            "Envelope requires API 36+ and device support"
          } else {
            "Device does not support envelope effects"
          }
          boundedPredefined(
            VibrationEffect.EFFECT_TICK, PREDEFINED_MS.getValue("tick"), maxDur, minOf(deviceTopTier(), 3),
            listOf(reason),
          )
        } else {
          val eff = buildEnvelopeEffect(effectObj, maxDur)
          Built(eff, 4, envelopeDurationMs(effectObj))
        }
      }

      else -> throw IllegalArgumentException("Unknown effect type: $type")
    }
  }

  /**
   * Composition of primitives only. A primitive the motor lacks is swapped for its nearest neighbour,
   * dropped when it has none, and every change is reported; with nothing left it plays a click.
   */
  private fun buildComposition(effectObj: JSObject, maxDur: Long): Built {
    val steps = getArray(effectObj, "steps")
    val support = primitiveSupport(true)
    val reasons = mutableListOf<String>()
    val comp = VibrationEffect.startComposition()
    var added = 0
    var total = 0L

    for (i in 0 until steps.length()) {
      val step = getObject(steps, i)
      val kind = step.getString("kind")
      if (kind != "primitive") {
        throw IllegalArgumentException(
          "steps[$i]: unsupported step kind `$kind`. Compositions are primitives only."
        )
      }
      val requested = step.getString("primitive").lowercase()
      if (requested !in PRIMITIVE_IDS) {
        throw IllegalArgumentException("steps[$i]: unknown primitive `$requested`")
      }
      val delay = if (step.present("delayMs")) step.getLong("delayMs").toInt().coerceAtLeast(0) else 0
      val scale = if (step.present("scale")) step.getDouble("scale").toFloat().coerceIn(0f, 1f) else 1f

      var id: String? = requested
      if (support[requested]?.first != true) {
        id = PRIMITIVE_NEIGHBOURS[requested]?.firstOrNull { support[it]?.first == true }
        if (id != null) {
          reasons.add("$requested missing on this motor → $id")
        } else {
          reasons.add("$requested missing on this motor and has no neighbour; step dropped")
          continue
        }
      }
      val stepMs = delay + (support[id]?.second ?: PRIMITIVE_MS.getValue(id)).toLong()
      if (total + stepMs > maxDur) {
        reasons.add("Truncated to $maxDur ms")
        break
      }
      comp.addPrimitive(mapPrimitive(id!!), scale, delay)
      added++
      total += stepMs
    }

    if (added == 0) {
      reasons.add("No playable steps; played a click")
      return boundedPredefined(
        VibrationEffect.EFFECT_CLICK, PREDEFINED_MS.getValue("click"), maxDur, minOf(deviceTopTier(), 3), reasons,
      )
    }
    return Built(comp.compose(), 3, total, reasons)
  }

  /**
   * `createPredefined` arrived in API 29; older releases play a one-shot pulse of the same length.
   */
  private fun predefinedEffect(constant: Int, pulseMs: Long): VibrationEffect {
    if (Build.VERSION.SDK_INT >= 29) return VibrationEffect.createPredefined(constant)
    return VibrationEffect.createOneShot(pulseMs, VibrationEffect.DEFAULT_AMPLITUDE)
  }

  /**
   * A predefined effect has a fixed length; when the remaining cap is shorter it is replaced by a
   * one-shot pulse that fits, and the change is reported.
   */
  private fun boundedPredefined(
    constant: Int,
    ms: Long,
    maxDur: Long,
    tier: Int,
    reasons: List<String> = emptyList(),
  ): Built {
    if (ms <= maxDur) return Built(predefinedEffect(constant, ms), tier, ms, reasons)
    return Built(
      VibrationEffect.createOneShot(maxDur, VibrationEffect.DEFAULT_AMPLITUDE),
      minOf(tier, 1),
      maxDur,
      reasons + "Truncated to $maxDur ms",
    )
  }

  private fun deviceTopTier(): Int {
    val hasVibrator = vibrator.hasVibrator()
    val anyPrimitive = primitiveSupport(hasVibrator).values.any { it.first }
    val amplitude = hasVibrator && vibrator.hasAmplitudeControl()
    return topTier(hasVibrator, hasVibrator && envelopeEffectsSupported(), anyPrimitive, amplitude)
  }

  private fun envelopeDurationMs(effectObj: JSObject): Long {
    val points = getArray(effectObj, "controlPoints", "control_points")
    var total = 0L
    for (i in 0 until points.length()) total += getLong(getObject(points, i), "durationMs", "duration_ms")
    return total
  }

  private fun envelopeEffectsSupported(): Boolean {
    if (Build.VERSION.SDK_INT < 36) return false
    return runCatching { vibrator.areEnvelopeEffectsSupported() }.getOrDefault(false)
  }

  private fun envelopeInfo(): JSObject? {
    if (Build.VERSION.SDK_INT < 36) return null
    return runCatching {
      val info = vibrator.envelopeEffectInfo
      val out = JSObject()
      out.put("maxSize", info.maxSize)
      out.put("minControlPointDurationMs", info.minControlPointDurationMillis)
      out.put("maxControlPointDurationMs", info.maxControlPointDurationMillis)
      out.put("maxDurationMs", info.maxDurationMillis)
      val profile = vibrator.frequencyProfile
      if (profile != null) {
        val fp = JSObject()
        fp.put("minHz", profile.minFrequencyHz.toDouble())
        fp.put("maxHz", profile.maxFrequencyHz.toDouble())
        out.put("frequencyProfile", fp)
      }
      out
    }.getOrNull()
  }

  /**
   * The checks that do not depend on the hardware, so an invalid envelope is rejected the same way
   * on a device that falls back to a tick as on one that plays it.
   */
  private fun validateEnvelopeShape(effectObj: JSObject) {
    val points = getArray(effectObj, "controlPoints", "control_points")
    if (points.length() == 0) {
      throw IllegalArgumentException("controlPoints cannot be empty")
    }
    for (i in 0 until points.length()) {
      val p = getObject(points, i)
      if (!p.present("amplitude")) throw IllegalArgumentException("controlPoints[$i]: missing amplitude")
      val amplitude = p.getDouble("amplitude").toFloat()
      if (amplitude.isNaN() || amplitude < 0f || amplitude > 1f) {
        throw IllegalArgumentException("controlPoints[$i]: amplitude must be within 0..1")
      }
      checkFrequency(getDouble(p, "frequencyHz", "frequency_hz", i).toFloat(), null)
      if (getLong(p, "durationMs", "duration_ms") <= 0) {
        throw IllegalArgumentException("controlPoints[$i]: durationMs must be positive")
      }
    }
  }

  /**
   * Builds a waveform envelope from `controlPoints` (amplitude 0..1, frequencyHz, durationMs).
   * Validates against device limits so callers get a clear INVALID_EFFECT error.
   */
  private fun buildEnvelopeEffect(effectObj: JSObject, maxDur: Long): VibrationEffect {
    if (Build.VERSION.SDK_INT < 36) throw IllegalStateException("Envelope requires API 36+")

    validateEnvelopeShape(effectObj)
    val points = getArray(effectObj, "controlPoints", "control_points")

    val info = vibrator.envelopeEffectInfo
    val profile = vibrator.frequencyProfile
    if (points.length() > info.maxSize) {
      throw IllegalArgumentException("controlPoints exceeds device maximum of ${info.maxSize}")
    }

    val builder = VibrationEffect.WaveformEnvelopeBuilder()
    val initial = when {
      effectObj.present("initialFrequencyHz") -> effectObj.getDouble("initialFrequencyHz")
      effectObj.present("initial_frequency_hz") -> effectObj.getDouble("initial_frequency_hz")
      else -> null
    }
    if (initial != null) {
      builder.setInitialFrequencyHz(checkFrequency(initial.toFloat(), profile))
    }

    var total = 0L
    for (i in 0 until points.length()) {
      val p = getObject(points, i)
      val amplitude = p.getDouble("amplitude").toFloat()
      val freq = checkFrequency(getDouble(p, "frequencyHz", "frequency_hz", i).toFloat(), profile)
      val dur = getLong(p, "durationMs", "duration_ms")
      if (dur < info.minControlPointDurationMillis || dur > info.maxControlPointDurationMillis) {
        throw IllegalArgumentException(
          "controlPoints[$i]: durationMs must be within " +
            "${info.minControlPointDurationMillis}..${info.maxControlPointDurationMillis}"
        )
      }
      total += dur
      builder.addControlPoint(amplitude, freq, dur)
    }

    val limit = minOf(maxDur, info.maxDurationMillis)
    if (total > limit) {
      throw IllegalArgumentException("envelope duration ${total}ms exceeds limit of ${limit}ms")
    }
    return builder.build()
  }

  private fun checkFrequency(hz: Float, profile: android.os.vibrator.VibratorFrequencyProfile?): Float {
    if (hz.isNaN() || hz <= 0f) throw IllegalArgumentException("frequencyHz must be positive")
    if (profile != null && (hz < profile.minFrequencyHz || hz > profile.maxFrequencyHz)) {
      throw IllegalArgumentException(
        "frequencyHz $hz outside device range ${profile.minFrequencyHz}..${profile.maxFrequencyHz}"
      )
    }
    return hz
  }

  private fun getDouble(obj: JSObject, primary: String, fallback: String, index: Int): Double {
    if (obj.has(primary)) return obj.getDouble(primary)
    if (obj.has(fallback)) return obj.getDouble(fallback)
    throw IllegalArgumentException("controlPoints[$index]: missing field `$primary`")
  }

  private fun capWaveformDuration(timings: LongArray, maxDur: Long): LongArray {
    var total = 0L
    val out = LongArray(timings.size)
    for (i in timings.indices) {
      val remain = (maxDur - total).coerceAtLeast(0)
      val v = timings[i].coerceAtLeast(0)
      val capped = v.coerceAtMost(remain)
      out[i] = capped
      total += capped
      if (total >= maxDur) {
        // zero out the rest
        for (j in i + 1 until timings.size) out[j] = 0
        break
      }
    }
    return out
  }

  private fun toLongArray(arr: JSArray): LongArray {
    val out = LongArray(arr.length())
    for (i in 0 until arr.length()) out[i] = arr.getLong(i)
    return out
  }

  private fun toIntArray(arr: JSArray): IntArray {
    val out = IntArray(arr.length())
    for (i in 0 until arr.length()) out[i] = arr.getInt(i)
    return out
  }

  private fun getArray(obj: JSObject, vararg keys: String): JSArray {
    for (key in keys) {
      if (!obj.has(key)) continue
      val raw = obj.get(key)
      when (raw) {
        is JSArray -> return raw
        is JSONArray -> return JSArray(raw.toString())
        else -> {
          val arr = JSArray.from(raw)
          if (arr != null) return arr
        }
      }
    }
    return JSArray()
  }

  private fun getLong(obj: JSObject, primary: String, fallback: String): Long {
    if (obj.has(primary)) return obj.getLong(primary)
    if (obj.has(fallback)) return obj.getLong(fallback)
    throw IllegalArgumentException("missing field `$primary`")
  }

  private fun getString(obj: JSObject, primary: String, fallback: String): String {
    if (obj.has(primary)) return obj.getString(primary)
    if (obj.has(fallback)) return obj.getString(fallback)
    throw IllegalArgumentException("missing field `$primary`")
  }

  private fun getObject(arr: JSArray, index: Int): JSObject {
    return JSObject.fromJSONObject(arr.getJSONObject(index))
  }

  private fun audioAttributesForUsage(usage: String, cfg: PluginConfigArgs): AudioAttributes {
    val u = usage.lowercase()

    val mappedUsage = when (u) {
      "alarm" -> AudioAttributes.USAGE_ALARM
      "notification" -> AudioAttributes.USAGE_NOTIFICATION
      "media" -> AudioAttributes.USAGE_MEDIA
      else -> AudioAttributes.USAGE_ASSISTANCE_SONIFICATION
    }

    return AudioAttributes.Builder()
      .setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION)
      .setUsage(mappedUsage)
      .build()
  }

  private fun mapPredefinedEffect(id: String): Int {
    return when (id.lowercase()) {
      "click" -> VibrationEffect.EFFECT_CLICK
      "double_click" -> VibrationEffect.EFFECT_DOUBLE_CLICK
      "tick" -> VibrationEffect.EFFECT_TICK
      "heavy_click" -> VibrationEffect.EFFECT_HEAVY_CLICK
      else -> throw IllegalArgumentException("Unknown predefined effect `$id`")
    }
  }

  private fun mapPrimitive(id: String): Int {
    return when (id.lowercase()) {
      "tick" -> VibrationEffect.Composition.PRIMITIVE_TICK
      "low_tick" -> VibrationEffect.Composition.PRIMITIVE_LOW_TICK
      "click" -> VibrationEffect.Composition.PRIMITIVE_CLICK
      "thud" -> VibrationEffect.Composition.PRIMITIVE_THUD
      "spin" -> VibrationEffect.Composition.PRIMITIVE_SPIN
      "quick_rise" -> VibrationEffect.Composition.PRIMITIVE_QUICK_RISE
      "slow_rise" -> VibrationEffect.Composition.PRIMITIVE_SLOW_RISE
      else -> VibrationEffect.Composition.PRIMITIVE_CLICK
    }
  }

  private fun primitiveConstant(id: String): Int = mapPrimitive(id)

  private fun predefinedConstant(id: String): Int = mapPredefinedEffect(id)

  private fun getVibrator(ctx: Context): Vibrator {
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      val vm = ctx.getSystemService(VibratorManager::class.java)
      vm.defaultVibrator
    } else {
      @Suppress("DEPRECATION")
      ctx.getSystemService(Context.VIBRATOR_SERVICE) as Vibrator
    }
  }
}
