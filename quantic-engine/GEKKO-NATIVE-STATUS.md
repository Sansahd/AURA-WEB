# GEKKO NOIR natif — état réel (20 octobre 2026)

Moteur : Quantic Engine Q0.5 (Rust/Boa), aucune intégration Chromium/Electron/CEF/WebView2.
UI native : egui/eframe, thème noir, logo origami, commande de fenêtre, recherche, onglets indépendants, navigation, historique par onglet, favoris locaux.
Le code Chromium présent dans d'autres branches n'est pas exécuté par ce binaire.

## Limites : ne pas annoncer « 100 % »
Moteur expérimental : insuffisant pour Netflix, Twitch, YouTube moderne/DRM, WebAssembly, CSS avancé, accessibilité et compatibilité Web générale.
L'ouverture de site peut bloquer le fil UI pendant le chargement ; une architecture asynchrone est encore à réaliser.
L'icône Windows native de l'exécutable, la navigation avancée et les raccourcis doivent être testés sur PC.
Ne pas basculer la version Electron/Chromium en production avant validation utilisateur du navigateur natif.

## Contrôles automatiques
cargo tree (dépendances interdites), cargo fmt, cargo test --bin qglide, cargo build --release --bin qglide.
Ces contrôles vérifient la compilation native mais ne prouvent pas la compatibilité avec tous les sites ni l'absence de bugs UI.
