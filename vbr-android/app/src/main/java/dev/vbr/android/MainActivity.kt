package dev.vbr.android

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.content.SharedPreferences
import android.graphics.Color
import android.graphics.Rect
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsAnimation
import android.view.inputmethod.InputMethodManager
import android.webkit.JavascriptInterface
import android.webkit.WebChromeClient
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.Toast
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

class MainActivity : Activity() {
    private var nativeOk = false
    private lateinit var web: WebView
    private lateinit var prefs: SharedPreferences
    private var pendingSaveText: String = ""

    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        prefs = getSharedPreferences("tide", MODE_PRIVATE)
        nativeOk = try {
            System.loadLibrary("vbr_android")
            val tccDir = File(codeCacheDir, "tcc").apply { mkdirs() }
            for (name in listOf("libtcc1.a", "runmain.o")) {
                assets.open("tcc/$name").use { inp ->
                    File(tccDir, name).outputStream().use { out -> inp.copyTo(out) }
                }
            }
            VbrNative.setTccDir(tccDir.absolutePath)
            android.util.Log.i("VBR", "tcc dir ${tccDir.absolutePath}")
            true
        } catch (e: UnsatisfiedLinkError) {
            android.util.Log.e("VBR", "loadLibrary(vbr_android) failed", e)
            false
        } catch (e: Exception) {
            android.util.Log.e("VBR", "tcc runtime install failed", e)
            false
        }

        web = WebView(this)
        web.setBackgroundColor(Color.parseColor("#0000AA"))
        setContentView(web)
        web.settings.javaScriptEnabled = true
        web.settings.domStorageEnabled = true
        web.settings.allowFileAccess = true
        web.settings.cacheMode = WebSettings.LOAD_NO_CACHE
        web.settings.mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
        web.settings.setSupportZoom(false)
        web.settings.builtInZoomControls = false
        @Suppress("DEPRECATION")
        web.settings.allowFileAccessFromFileURLs = true
        web.isFocusable = true
        web.isFocusableInTouchMode = true
        web.webChromeClient = WebChromeClient()
        web.webViewClient = object : WebViewClient() {
            override fun onPageFinished(view: WebView?, url: String?) {
                notifyImeHeight()
            }
        }
        web.addJavascriptInterface(Bridge(), "Vbr")
        web.loadUrl("file:///android_asset/index.html")
        hookIme()

        if (!nativeOk) {
            Toast.makeText(
                this,
                "Native library missing — rebuild with scripts/build-native.sh (NDK).",
                Toast.LENGTH_LONG,
            ).show()
        }
    }

    private fun hookIme() {
        val root = window.decorView
        val listen: (View, WindowInsets) -> WindowInsets = { v, insets ->
            notifyImeHeight(imeOverlapPx(insets))
            v.onApplyWindowInsets(insets)
        }
        if (Build.VERSION.SDK_INT >= 30) {
            root.setOnApplyWindowInsetsListener { v, insets -> listen(v, insets) }
            web.setOnApplyWindowInsetsListener { v, insets -> listen(v, insets) }
            val anim = {
                object : WindowInsetsAnimation.Callback(
                    WindowInsetsAnimation.Callback.DISPATCH_MODE_CONTINUE_ON_SUBTREE,
                ) {
                    override fun onProgress(
                        insets: WindowInsets,
                        runningAnimations: MutableList<WindowInsetsAnimation>,
                    ): WindowInsets {
                        notifyImeHeight(imeOverlapPx(insets))
                        return insets
                    }
                }
            }
            root.setWindowInsetsAnimationCallback(anim())
            web.setWindowInsetsAnimationCallback(anim())
        }
        root.viewTreeObserver.addOnGlobalLayoutListener { notifyImeHeight() }
    }

    /**
     * How much of the WebView is covered by the IME. `ime().bottom` is from the
     * window edge and includes the 3-button nav; the WebView already sits above
     * that bar, so using the raw inset leaves a nav-sized gap.
     */
    private fun imeOverlapPx(insets: WindowInsets? = null): Int {
        val root = window.decorView
        if (Build.VERSION.SDK_INT >= 30) {
            val ins = insets ?: root.rootWindowInsets
            if (ins != null) {
                val ime = ins.getInsets(WindowInsets.Type.ime()).bottom
                if (ime <= 0) return 0
                if (!::web.isInitialized) return ime
                val loc = IntArray(2)
                web.getLocationInWindow(loc)
                val below = (root.height - loc[1] - web.height).coerceAtLeast(0)
                return (ime - below).coerceAtLeast(0)
            }
        }
        val visible = Rect()
        root.getWindowVisibleDisplayFrame(visible)
        return (root.height - visible.bottom).coerceAtLeast(0)
    }

    private fun notifyImeHeight(androidPx: Int = imeOverlapPx()) {
        if (!::web.isInitialized) return
        web.evaluateJavascript(
            "window.onImeInset&&window.onImeInset($androidPx)",
            null,
        )
    }

    @Deprecated("Deprecated in Java")
    override fun onBackPressed() {
        web.evaluateJavascript("window.tideBack && window.tideBack()") { result ->
            if (result == "true") {
                @Suppress("DEPRECATION")
                super.onBackPressed()
            }
        }
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        @Suppress("DEPRECATION")
        super.onActivityResult(requestCode, resultCode, data)
        if (resultCode != RESULT_OK || data?.data == null) return
        val uri = data.data!!
        when (requestCode) {
            REQ_OPEN -> {
                takePersist(uri, read = true, write = false)
                val obj = JSONObject()
                    .put("uri", uri.toString())
                    .put("name", queryName(uri))
                callJs("onPickedFile", obj)
            }
            REQ_TREE -> {
                takePersist(uri, read = true, write = true)
                val units = listVbrInTree(uri)
                val obj = JSONObject()
                    .put("uri", uri.toString())
                    .put("name", treeName(uri))
                    .put("units", units)
                callJs("onPickedProject", obj)
            }
            REQ_CREATE -> {
                takePersist(uri, read = true, write = true)
                val err = writeUri(uri, pendingSaveText)
                if (err != "ok") {
                    js("onNativeError(${JSONObject.quote(err)})")
                    return
                }
                val obj = JSONObject()
                    .put("uri", uri.toString())
                    .put("name", queryName(uri))
                callJs("onSavedAs", obj)
            }
        }
    }

    private fun callJs(fn: String, obj: JSONObject) {
        js("$fn($obj)")
    }

    private fun js(code: String) {
        web.post { web.evaluateJavascript(code, null) }
    }

    private fun takePersist(uri: Uri, read: Boolean, write: Boolean) {
        var flags = 0
        if (read) flags = flags or Intent.FLAG_GRANT_READ_URI_PERMISSION
        if (write) flags = flags or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        try {
            contentResolver.takePersistableUriPermission(uri, flags)
        } catch (_: SecurityException) {
            // Some providers only grant for the session.
        }
    }

    private fun queryName(uri: Uri): String {
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                val n = c.getString(0)
                if (!n.isNullOrBlank()) return n
            }
        }
        return uri.lastPathSegment?.substringAfterLast('/') ?: "file.vbr"
    }

    private fun treeName(uri: Uri): String {
        val id = try {
            DocumentsContract.getTreeDocumentId(uri)
        } catch (_: Exception) {
            uri.lastPathSegment
        }
        return id?.substringAfterLast(':')?.substringAfterLast('/') ?: "project"
    }

    private fun listVbrInTree(tree: Uri): JSONArray {
        val arr = JSONArray()
        val children = try {
            DocumentsContract.buildChildDocumentsUriUsingTree(
                tree,
                DocumentsContract.getTreeDocumentId(tree),
            )
        } catch (_: Exception) {
            return arr
        }
        val projection = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
        )
        try {
            contentResolver.query(children, projection, null, null, null)?.use { c ->
                val idI = c.getColumnIndex(DocumentsContract.Document.COLUMN_DOCUMENT_ID)
                val nameI = c.getColumnIndex(DocumentsContract.Document.COLUMN_DISPLAY_NAME)
                val mimeI = c.getColumnIndex(DocumentsContract.Document.COLUMN_MIME_TYPE)
                val rows = mutableListOf<JSONObject>()
                while (c.moveToNext()) {
                    val name = c.getString(nameI) ?: continue
                    val mime = c.getString(mimeI) ?: ""
                    if (mime == DocumentsContract.Document.MIME_TYPE_DIR) continue
                    if (!name.endsWith(".vbr", ignoreCase = true)) continue
                    val docUri = DocumentsContract.buildDocumentUriUsingTree(tree, c.getString(idI))
                    rows.add(JSONObject().put("name", name).put("uri", docUri.toString()))
                }
                rows.sortWith { a, b ->
                    val an = a.getString("name")
                    val bn = b.getString("name")
                    val am = an.equals("main.vbr", true)
                    val bm = bn.equals("main.vbr", true)
                    when {
                        am && !bm -> -1
                        !am && bm -> 1
                        else -> an.compareTo(bn, ignoreCase = true)
                    }
                }
                for (r in rows) arr.put(r)
            }
        } catch (e: Exception) {
            android.util.Log.w("VBR", "list tree", e)
        }
        return arr
    }

    private fun programsDir(): File = File(filesDir, "programs").apply { mkdirs() }

    private fun readUri(uri: Uri): String {
        return try {
            contentResolver.openInputStream(uri)?.bufferedReader()?.use { it.readText() } ?: ""
        } catch (e: Exception) {
            android.util.Log.e("VBR", "readUri", e)
            ""
        }
    }

    private fun writeUri(uri: Uri, text: String): String {
        return try {
            contentResolver.openOutputStream(uri, "wt")?.bufferedWriter()?.use { it.write(text) }
                ?: return "Cannot write (no stream)"
            "ok"
        } catch (e: Exception) {
            e.message ?: e.toString()
        }
    }

    inner class Bridge {
        @JavascriptInterface
        fun hasNative(): Boolean = nativeOk

        @JavascriptInterface
        fun compile(source: String): String =
            if (nativeOk) VbrNative.compile(source) else missing("compile")

        @JavascriptInterface
        fun run(source: String): String {
            android.util.Log.i("VBR", "run() ${source.length} chars")
            return try {
                if (nativeOk) {
                    val out = VbrNative.run(source)
                    android.util.Log.i("VBR", "run() returned ${out.length} chars")
                    out
                } else missing("run")
            } catch (e: Throwable) {
                android.util.Log.e("VBR", "run() crashed", e)
                JSONObject()
                    .put("stage", "compile")
                    .put("success", false)
                    .put("stdout", "")
                    .put("stderr", e.toString())
                    .put("code", "")
                    .put("diagnostics", JSONArray())
                    .put("line_map", JSONArray())
                    .put("surface", JSONObject.NULL)
                    .toString()
            }
        }

        @JavascriptInterface
        fun complete(source: String, line: Int, col: Int): String =
            if (nativeOk) VbrNative.complete(source, line, col) else "[]"

        @JavascriptInterface
        fun hover(source: String, line: Int, col: Int): String =
            if (nativeOk) VbrNative.hover(source, line, col) else ""

        @JavascriptInterface
        fun screenStart(source: String): String =
            if (nativeOk) VbrNative.screenStart(source) else missing("screenStart")

        @JavascriptInterface
        fun screenDispatch(event: String): String =
            if (nativeOk) VbrNative.screenDispatch(event) else missing("screenDispatch")

        @JavascriptInterface
        fun screenStop() {
            if (nativeOk) VbrNative.screenStop()
        }

        @JavascriptInterface
        fun listExamples(): String {
            val names = assets.list("examples")?.sorted() ?: emptyList()
            val arr = JSONArray()
            for (n in names) {
                if (n.endsWith(".vbr")) arr.put(n.removeSuffix(".vbr"))
            }
            return arr.toString()
        }

        @JavascriptInterface
        fun loadExample(name: String): String {
            val file = name.removeSuffix(".vbr") + ".vbr"
            return assets.open("examples/$file").bufferedReader().use { it.readText() }
        }

        @JavascriptInterface
        fun saveProgram(name: String, source: String): String {
            val safe = File(name).name.replace(Regex("[^A-Za-z0-9._-]"), "_")
            File(programsDir(), if (safe.endsWith(".vbr")) safe else "$safe.vbr").writeText(source)
            return "ok"
        }

        @JavascriptInterface
        fun listSaved(): String {
            val arr = JSONArray()
            programsDir().listFiles()?.sortedBy { it.name }?.forEach { arr.put(it.name) }
            return arr.toString()
        }

        @JavascriptInterface
        fun loadSaved(name: String): String {
            val f = File(programsDir(), File(name).name)
            return if (f.isFile) f.readText() else ""
        }

        @JavascriptInterface
        fun pickOpenFile() {
            runOnUiThread {
                val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "*/*"
                    putExtra(
                        Intent.EXTRA_MIME_TYPES,
                        arrayOf("text/plain", "application/octet-stream", "*/*"),
                    )
                    addFlags(
                        Intent.FLAG_GRANT_READ_URI_PERMISSION or
                            Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION,
                    )
                }
                @Suppress("DEPRECATION")
                startActivityForResult(intent, REQ_OPEN)
            }
        }

        @JavascriptInterface
        fun pickProject() {
            runOnUiThread {
                val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                    addFlags(
                        Intent.FLAG_GRANT_READ_URI_PERMISSION or
                            Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
                            Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION or
                            Intent.FLAG_GRANT_PREFIX_URI_PERMISSION,
                    )
                }
                @Suppress("DEPRECATION")
                startActivityForResult(intent, REQ_TREE)
            }
        }

        @JavascriptInterface
        fun pickSaveAs(name: String, text: String) {
            pendingSaveText = text
            runOnUiThread {
                val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "text/plain"
                    putExtra(Intent.EXTRA_TITLE, name.ifBlank { "NONAME.VBR" })
                    addFlags(
                        Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
                            Intent.FLAG_GRANT_READ_URI_PERMISSION or
                            Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION,
                    )
                }
                @Suppress("DEPRECATION")
                startActivityForResult(intent, REQ_CREATE)
            }
        }

        @JavascriptInterface
        fun readUri(uri: String): String {
            return try {
                this@MainActivity.readUri(Uri.parse(uri))
            } catch (e: Exception) {
                ""
            }
        }

        @JavascriptInterface
        fun writeUri(uri: String, text: String): String {
            return try {
                this@MainActivity.writeUri(Uri.parse(uri), text)
            } catch (e: Exception) {
                e.message ?: e.toString()
            }
        }

        @JavascriptInterface
        fun persistSession(json: String) {
            prefs.edit().putString("session", json).apply()
        }

        @JavascriptInterface
        fun restoreSession(): String = prefs.getString("session", "") ?: ""

        @JavascriptInterface
        fun hideKeyboard() {
            runOnUiThread {
                val imm = getSystemService(InputMethodManager::class.java)
                imm?.hideSoftInputFromWindow(web.windowToken, 0)
            }
        }

        @JavascriptInterface
        fun showKeyboard() {
            runOnUiThread {
                web.requestFocus()
                val imm = getSystemService(InputMethodManager::class.java)
                imm?.showSoftInput(web, InputMethodManager.SHOW_IMPLICIT)
            }
        }

        @JavascriptInterface
        fun finishApp() {
            runOnUiThread { finish() }
        }

        private fun missing(op: String): String {
            val msg =
                "Native library not loaded. Build the .so with vbr-android/scripts/build-native.sh (needs the Android NDK) and reinstall the APK."
            return JSONObject()
                .put("has_errors", true)
                .put("success", false)
                .put("stage", "compile")
                .put("code", "")
                .put("stdout", "")
                .put("stderr", msg)
                .put("diagnostics", JSONArray())
                .put("line_map", JSONArray())
                .put("blocked", msg)
                .put("surface", JSONObject.NULL)
                .toString()
        }
    }

    companion object {
        private const val REQ_OPEN = 1
        private const val REQ_TREE = 2
        private const val REQ_CREATE = 3
    }
}

object VbrNative {
    external fun setTccDir(path: String)
    external fun compile(source: String): String
    external fun run(source: String): String
    external fun complete(source: String, line: Int, col: Int): String
    external fun hover(source: String, line: Int, col: Int): String
    external fun screenStart(source: String): String
    external fun screenDispatch(event: String): String
    external fun screenStop()
}
