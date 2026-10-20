# GEKKO NOIR natif — Miroir noir / LED

Le code natif utilise Rust + eframe/egui + Quantic Engine/Boa ; pas d'Electron, Chromium, CEF, WebView2 ou de CSS d'un navigateur embarqué.

- Barre noire à reflet peint natif (trois bandes spéculaires et une base ajustable). **Ne pas présenter ceci comme du flou/translucidité du bureau Windows** : la fenêtre a un fond opaque ; le miroir est un effet dessiné par egui.
- Contrôles noir/anthracite, texte blanc. Halo LED sur survol et focus autour des contrôles, onglets inclus.
- Couleur LED (par défaut blanc), intensité, profondeur du miroir et réduction des animations dans Persona (engrenage de la barre).
- Préférences JSON locales dans GEKKO/Native/appearance.json ; favoris dans favorites.json.
- Le moteur Quantic Engine expérimental n'assure pas la compatibilité Web intégrale : vidéo/DRM, WebAssembly, CSS et accessibilité nécessitent du développement. Le portage ne modifie pas le moteur lui-même.
- La CI vérifie dépendances Chromium interdites, tests Rust et build Windows natif. Pas de revendication « 100 % fonctionnel » tant que les tests sur ordinateur et sites réels manquent.
