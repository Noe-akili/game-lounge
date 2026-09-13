#!/usr/bin/env bash
# Injecte la gestion des insets edge-to-edge dans le projet Android GÉNÉRÉ par
# Tauri (gen/android). Le projet n'existe pas dans le dépôt : ce patch est
# exécuté par le workflow GitHub Actions APRÈS `tauri android init`.
#
# Problème : à partir d'Android 15 (targetSdk 35/36), l'app dessine
# edge-to-edge et le WebView passe SOUS la barre de statut -> le header de
# l'app est recouvert par l'heure/la batterie.
#
# Solution (guide Android "Make WebViews edge-to-edge", cas "app n'ownant PAS
# le contenu natif") : envelopper le WebView dans un conteneur natif et lui
# appliquer les insets en PADDING -> l'interface de l'app commence SOUS la
# barre de statut, comme une app classique.
#
# Idempotent : relancer le script ne duplique rien.
set -euo pipefail

GEN_DIR="$(cd "$(dirname "$0")/.." && pwd)/server-rust/src-tauri/gen/android"
PKG_DIR="$GEN_DIR/app/src/main/java/com/gamelounge/android"

if [ ! -d "$GEN_DIR" ]; then
  echo "ERREUR: $GEN_DIR introuvable (exécuter après 'tauri android init')" >&2
  exit 1
fi
mkdir -p "$PKG_DIR"

# ---- 1. EdgeInsetActivity.kt : applique le padding des barres système au conteneur du WebView
#    + pont PDF "GameLoungePdf" (SAF : ACTION_OPEN_DOCUMENT_TREE au premier export,
#    URI persistée, écriture du PDF base64 dans le dossier choisi). ----
cat > "$PKG_DIR/EdgeInsetActivity.kt" <<'KOTLIN'
package com.gamelounge.android

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.DocumentsContract
import android.util.Base64
import android.webkit.JavascriptInterface
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * 1) Edge-to-edge : le conteneur natif du WebView reçoit les barres système en
 *    padding (guide Android "Make WebViews edge-to-edge") — l'interface de
 *    l'app commence SOUS la barre de statut.
 * 2) Pont PDF : expose GameLoungePdf.save(filename, base64) au WebView. Au
 *    premier export, ouvre le sélecteur de dossier système (SAF) et persiste
 *    l'URI ; les exports suivants écrivent directement dans ce dossier.
 */
abstract class EdgeInsetActivity : TauriActivity() {
    private var lastTop = -1
    private var lastBottom = -1

    private companion object { const val PDF_TREE_REQ = 47201 }

    private var pdfLatch: CountDownLatch? = null
    private var pdfTreeUri: Uri? = null

    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        applyInsets(webView)
        webView.addJavascriptInterface(PdfBridge(this), "GameLoungePdf")
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == PDF_TREE_REQ) {
            pdfTreeUri = if (resultCode == RESULT_OK) data?.data else null
            pdfTreeUri?.let {
                try {
                    contentResolver.takePersistableUriPermission(
                        it,
                        Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                    )
                } catch (_: SecurityException) {
                }
            }
            pdfLatch?.countDown()
        }
    }

    private fun applyInsets(webView: WebView) {
        val container: FrameLayout? = webView.parent as? FrameLayout
        if (container == null) {
            // Structure inattendue : sécurité — ne PAS redessiner sous la barre.
            android.util.Log.w("EdgeInsets", "conteneur WebView inattendu, insets non appliqués")
            return
        }
        ViewCompat.setOnApplyWindowInsetsListener(container) { _, windowInsets ->
            val insets = windowInsets.getInsets(
                WindowInsetsCompat.Type.systemBars()
                        or WindowInsetsCompat.Type.displayCutout()
            )
            if (insets.top != lastTop || insets.bottom != lastBottom) {
                lastTop = insets.top
                lastBottom = insets.bottom
                container.setPadding(0, insets.top, 0, insets.bottom)
            }
            // On laisse les insets continuer de circuler (aucune autre vue ne
            // réapplique de padding) : compatible avec toutes les versions d'Android.
            windowInsets
        }
        container.requestApplyInsets()
    }

    /** Écrit le PDF (base64) dans le dossier SAF choisi. Retourne l'URI écrite. */
    fun savePdf(name: String, base64: String): String? {
        val tree = ensureTreeUri() ?: return null
        val resolver = contentResolver
        val rootDoc = DocumentsContract.buildDocumentUriUsingTree(
            tree, DocumentsContract.getTreeDocumentId(tree)
        )
        // Remplace un fichier du même nom (pas de "facture (1).pdf" à chaque export).
        findChild(resolver, tree, rootDoc, name)?.let { existing ->
            try { DocumentsContract.deleteDocument(resolver, existing) } catch (_: Exception) {}
        }
        val created = DocumentsContract.createDocument(resolver, rootDoc, "application/pdf", name)
            ?: return null
        resolver.openOutputStream(created)?.use { os ->
            os.write(Base64.decode(base64, Base64.DEFAULT))
            os.flush()
        } ?: return null
        return created.toString()
    }

    private fun ensureTreeUri(): Uri? {
        val prefs = getSharedPreferences("gl_pdf_saf", Context.MODE_PRIVATE)
        prefs.getString("tree_uri", null)?.let { return Uri.parse(it) }
        // Premier export : choix du dossier (bloquant — appelé depuis le thread
        // JavaBridge de WebView, pas depuis l'UI thread).
        val latch = CountDownLatch(1)
        pdfLatch = latch
        pdfTreeUri = null
        runOnUiThread {
            try {
                startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT_TREE), PDF_TREE_REQ)
            } catch (_: Exception) {
                latch.countDown()
            }
        }
        latch.await(3, TimeUnit.MINUTES)
        pdfLatch = null
        val uri = pdfTreeUri ?: return null
        prefs.edit().putString("tree_uri", uri.toString()).apply()
        return uri
    }

    private fun findChild(
        resolver: android.content.ContentResolver,
        tree: Uri,
        parentDoc: Uri,
        name: String
    ): Uri? {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(
            parentDoc, DocumentsContract.getDocumentId(parentDoc)
        )
        resolver.query(
            children,
            arrayOf(
                DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME
            ),
            null, null, null
        )?.use { c ->
            while (c.moveToNext()) {
                if (name.equals(c.getString(1), ignoreCase = true)) {
                    return DocumentsContract.buildDocumentUriUsingTree(tree, c.getString(0))
                }
            }
        }
        return null
    }

    /** Pont JS exposé sous le nom global "GameLoungePdf". */
    private class PdfBridge(private val activity: EdgeInsetActivity) {
        @JavascriptInterface
        fun save(filename: String?, base64: String?): String? {
            if (base64.isNullOrBlank()) return null
            val name = (filename ?: "facture.pdf")
                .substringAfterLast('/')
                .substringAfterLast('\\')
                .ifBlank { "facture.pdf" }
            return try {
                activity.savePdf(name, base64)
            } catch (_: Exception) {
                null
            }
        }
    }
}
KOTLIN

# ---- 2. MainActivity.kt : étend EdgeInsetActivity (au lieu de TauriActivity) ----
MAIN_ACTIVITY="$PKG_DIR/MainActivity.kt"
cat > "$MAIN_ACTIVITY" <<'KOTLIN'
package com.gamelounge.android

import android.os.Bundle

class MainActivity : EdgeInsetActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
    }
}
KOTLIN

echo "✅ Patch edge-to-edge + pont PDF appliqué :"
echo "   - $PKG_DIR/EdgeInsetActivity.kt (insets + GameLoungePdf.save via SAF)"
echo "   - $MAIN_ACTIVITY (extends EdgeInsetActivity)"