# Réduire la taille de l'APK (Game Lounge Android)

Aide-mémoire : ce qui pèse dans l'application et comment livrer le plus léger
possible, sans rien perdre en fonctionnalités.

## 1. Une seule architecture : `arm64-v8a`

Par défaut, Tauri compile pour les quatre architectures (aarch64, armv7, i686,
x86_64) et les met toutes dans le même APK. C'est le premier gros gaspillage :

| Architecture  | Qui l'utilise                                   | À garder ?          |
|---------------|--------------------------------------------------|---------------------|
| `arm64-v8a`   | Quasiment tous les téléphones vendus depuis 2015 | **Oui, la seule**   |
| `armeabi-v7a` | Vieux téléphones 32 bits                         | Seulement si besoin |
| `x86` / `x86_64` | Émulateurs, très rares tablettes              | Non                 |

Commandes (depuis le dossier `server-rust`) :

```bash
npm run android:build            # APK arm64-v8a uniquement  <- recommandé
npm run android:build:split      # un APK par architecture (arm64 + armv7)
npm run android:build:aab        # AAB arm64 pour le Play Store
npm run android:build:universel  # ancien comportement : toutes les architectures
```

Pour le Play Store, préférer l'AAB : Google découpe lui-même le téléchargement
par architecture, l'utilisateur ne reçoit que sa part.

## 2. Compilation Rust optimisée pour la taille

Dans `server-rust/src-tauri/Cargo.toml`, le profil `release` utilise déjà
`opt-level = "z"`, `lto`, `codegen-units = 1` et `strip`. Ont été ajoutés :

- `incremental = false` : pas d'objets intermédiaires dans la bibliothèque finale ;
- `[profile.release.package."*"]` : les dépendances (SQLite, tokio, rustls…) sont
  elles aussi compilées pour la taille — c'est là que se trouve le plus gros ;
- `[profile.release.build-override]` : les scripts de build ne partent pas dans
  l'APK, donc on les compile sans optimisation (compilation plus rapide).

À NE PAS faire : `panic = "abort"`. Le code utilise `catch_unwind` (hachage des
mots de passe, démarrage, watcher de sessions) ; avec `abort`, la moindre panique
fermerait brutalement l'application au lieu d'être rattrapée.

## 3. Frontend découpé (le vrai gain au démarrage)

Avant, `vite.config.mobile.ts` forçait `inlineDynamicImports` : tout le frontend
partait dans un seul fichier de ~780 ko, chargé entièrement avant le premier
écran. Maintenant le code est découpé :

- au démarrage : noyau Vue + librairies + écran affiché (~330 ko) ;
- à la demande : chaque écran d'administration, la documentation, et surtout
  `chart.js` (~180 ko) qui ne sert que dans les rapports.

L'APK ne rétrécit pas beaucoup (mêmes octets au total), mais le premier
affichage est nettement plus rapide et la mémoire utilisée plus faible — c'est
ce qui se voit sur un téléphone d'entrée de gamme.

Si jamais la WebView d'un appareil refusait les imports dynamiques, on revient à
l'ancien comportement en remettant dans `vite.config.mobile.ts` :

```ts
output: { inlineDynamicImports: true, entryFileNames: 'assets/app.js', assetFileNames: 'assets/app.[ext]' }
```

## 4. Pistes suivantes (non faites ici)

- `motion-v` (~126 ko) n'est utilisé que pour quelques animations : le remplacer
  par des transitions CSS supprimerait la dépendance.
- Trois algorithmes de hachage sont embarqués (`scrypt`, `bcrypt`, `argon2`) pour
  rester compatible avec les anciens comptes. Le jour où tous les mots de passe
  sont ré-enregistrés, deux dépendances peuvent partir.
- Les icônes `lucide-vue-next` sont déjà importées une par une (bon réflexe) :
  continuer ainsi, jamais d'import global.
