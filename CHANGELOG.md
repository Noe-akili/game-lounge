# Journal des versions — Game Lounge

Toutes les versions notables de l'application Android (APK signé). Format inspiré de
[Keep a Changelog](https://keepachangelog.com/fr/1.1.0/).

## [1.1.0] - 2026-09-24

- Secrets retirés du dépôt : plus de mot de passe cloud ni de secret JWT partagé dans le code
- JWT unique par appareil : un appareil volé ne peut plus forger de jeton (clé aléatoire de 32 octets, générée localement)
- Synchronisation active par défaut sur tous les appareils, avec état visible et bouton Réessayer (Paramètres)
- Alerte automatique si des ventes attendent depuis plus de 24 h sans partir au cloud
- Rapports calculés en SQL : plus de troncature à 5 000 lignes, CA et bénéfices exacts
- URL cloud modifiable dans l'application (Admin > Paramètres > Synchronisation), sans reconstruire l'APK
- Publication automatique de l'APK signé dans les Releases GitHub (tag vX.Y.Z)

