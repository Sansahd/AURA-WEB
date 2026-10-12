# GEKKO Search 2.5 — specification de realisation

Objectif: reproduire le style glass premium de la maquette approuvee en UI reelle, responsive et alimentée par des sources.

Fonctions: barre de recherche GEKKO, filtres effectifs, onglets Tout/Images/Videos/Actualites, resultats reels avec source et navigation, volet documentaire Wikipedia facultatif, recommandations de requetes generiques, Search Assist reliant AURA sans pretendre avoir genere un resume.

Sources: Bing RSS et DuckDuckGo HTML pour le Web, Wikimedia Commons pour les images, Wikipedia pour une fiche documentaire; aucune API payante ni mesure de suivi. Chaque source affiche son attribution. Le moteur GEKKO n'a pas encore son propre index.

Securite: ressources chargees seulement par le processus principal, taille limitee, ressources image rasterisees en data URI, HTML non fiable jamais injecte dans le shell, politiques de navigation et IPC conservees.

A ne pas simuler: images de Spider-Man dans des resultats qui n'en fournissent pas, informations structurées Wikidata indisponibles, nombre de resultats invente, reponse IA generée si moteur AURA hors ligne.

Criteres: tests unitaires, smoke electron Windows, navigation clavier, mode image/video/news, responsive 700/1050px, aucune regression favoris et recherche rapide.
