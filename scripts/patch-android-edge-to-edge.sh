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

# ---- 1. EdgeInsetActivity.kt : applique le padding des barres système au conteneur du WebView ----
cat > "$PKG_DIR/EdgeInsetActivity.kt" <<'KOTLIN'
package com.gamelounge.android

import android.os.Bundle
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * Pousse le contenu de l'app SOUS la barre de statut : le conteneur natif du
 * WebView reçoit les barres système en padding (guide Android "Make WebViews
 * edge-to-edge" pour WebView affichant du contenu externe). L'app dessine
 * toujours edge-to-edge (fond sombre continu derrière la barre) mais son
 * interface commence en dessous.
 */
abstract class EdgeInsetActivity : TauriActivity() {
    private var lastTop = -1
    private var lastBottom = -1

    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        applyInsets(webView)
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

echo "✅ Patch edge-to-edge appliqué :"
echo "   - $PKG_DIR/EdgeInsetActivity.kt (padding des barres système sur le conteneur du WebView)"
echo "   - $MAIN_ACTIVITY (extends EdgeInsetActivity)"