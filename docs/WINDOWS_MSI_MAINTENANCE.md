# Contrat de maintenance MSI sous Windows

MicCamWatch vérifie séparément les fichiers, l’enregistrement du ProductCode
dans le profil d’origine et la découverte du même produit via son UpgradeCode.
Le cache MSI doit correspondre à la version, au produit et à sa famille.
Une lecture impossible, un produit concurrent ou un autre contexte empêche
les opérations mutatrices.

## États et transitions

| État observé | Action autorisée |
| --- | --- |
| Ancien produit et fichiers cohérents | Préparer la mise à jour, vérifier les artefacts, puis demander confirmation. |
| Transaction en cours ou interrompue | Conserver le journal et attendre un état MSI observable. Aucun rejeu automatique. |
| Nouveau produit et bundle cohérents, ancien absent | Valider la mise à niveau. |
| Mise à niveau refusée ou annulée, ancien produit cohérent | Conserver l’installation et qualifier à nouveau avant de retenter. |
| Rollback avec ancien produit et fichiers cohérents | Clore le journal comme restauration vérifiée. |
| Fichiers anciens intacts, ProductCode absent | Conserver `intervention_required`. Proposer le MSI d’origine vérifié en mode interactif. |
| Fichiers anciens intacts, produit seulement annoncé (`State=1`) sans index | Vérifier aussi le PackageCode contre le MSI authentifié. Seule la réinstallation interactive est proposée. |
| ProductCode rétabli dans le bon profil, index incomplet | Demander une confirmation distincte avant une désinstallation explicite par ProductCode. |
| Ancien produit absent après désinstallation ciblée | Vérifier l’absence avant de proposer une nouvelle installation du MSI d’origine. |
| Ancien produit, index, cache et fichiers cohérents | Valider la récupération et permettre les opérations normales. |
| État ambigu, artefact indisponible, refus UAC ou incohérence restante | Conserver `intervention_required`, le dernier état vérifié et les diagnostics pour une reprise explicite. |

Windows Installer reste seul responsable de ses écritures et d’UAC. Les
opérations partent du compte de l’installation. Le pilote vérifie la sortie
du tray et l’état des protections via les gardes Rust. La surveillance peut
être interrompue pendant la maintenance. Sauvegarder les données hors
du bundle avant de retirer un produit. Les journaux et la sauvegarde restent
accessibles après la fermeture de l’application.

La provenance repose sur les métadonnées HTTPS du dépôt GitHub officiel et
l’empreinte de l’artefact. Une empreinte immuable du MSI v0.21.0 admis permet
la récupération hors ligne de cette version. Pour les autres versions, une
copie locale dont la provenance ne peut plus être établie entraîne un refus.

## Reprendre une maintenance

Le CLI de mise à jour conserve le MSI précédent dans le dossier de
maintenance du profil avant toute transaction. Il affiche le chemin du
reçu et du pilote `maintenance.exe`. La consultation Installation dans
Flutter affiche le chemin de reprise lorsqu’un dossier MSI est en attente.
L’action s’exécute dans le compte Windows d’origine depuis un terminal
extérieur au dossier d’installation.

- Pour une mise à jour entamée par le CLI, lancer le pilote conservé avec
  `recover-update` pour vérifier l’état, ou `recover-update --repair`
  pour examiner et confirmer une opération de récupération.
- Après la migration directe depuis v0.21.0, si l’application installée
  est indisponible, extraire `mcw.exe` depuis le ZIP hybride officiel
  vérifié dans un dossier distinct. Lancer
  `mcw recover-msi C:\chemin\miccamwatch-windows-x86_64.msi` avec le
  MSI officiel v0.21.0 x64. Le CLI vérifie son SHA-256 et affiche la commande
  du pilote retenu. Ce parcours couvre l’installation standard v0.21.0
  dont le contenu d’origine est intact.
- Le pilote propose une opération à la fois. Saisir `restore` pour une
  réinstallation interactive initiale, `remove` pour confirmer le retrait
  du ProductCode déjà rétabli, ou `reinstall` après vérification de l’absence.
  Examiner à nouveau l’état avec `recover-update --repair` entre deux étapes.
- Le dossier contient le journal MSI de chaque opération, les résultats
  vérifiés et une sauvegarde des données du profil avec l’état d’autostart.
  Un code de sortie manquant ou une demande de redémarrage impose une
  nouvelle observation après redémarrage. Les états ambigus restent en
  intervention manuelle.

## Preuves du laboratoire

Sur Windows 11 Pro x64 dans un compte standard, la mise à niveau normale
v0.21.0 → v0.22.0 (16 composants, IPC 10) et la désinstallation ont réussi.
La panne avant retrait a préservé l’ancien produit. La panne après retrait
a restauré les fichiers à l’identique tandis que les deux ProductCode
renvoyaient `1605` depuis l’observateur administrateur distinct. Depuis le
compte d’installation, l’ancienne version reste annoncée (`State=1`), avec
son PackageCode et sans chemin d’installation, cache ni index UpgradeCode.
Le journal MSI contient 79 refus d’écriture. Le résultat
correspond à la perte d’enregistrement observée avec le MSI minimal.

La réinstallation interactive du MSI d’origine avec UAC a renvoyé `0`
et restauré le ProductCode dans le compte standard. `MsiEnumRelatedProductsW`
ne le retrouvait toujours pas. La désinstallation ciblée par ProductCode,
le contrôle d’absence puis la réinstallation du même MSI ont rétabli
l’enregistrement et l’index. Une nouvelle mise à niveau et sa désinstallation
ont réussi, avec conservation des données de test.

Les reçus d’origine restent dans le laboratoire, avec les journaux MSI et
leurs empreintes. Ces observations précèdent l’intégration du mécanisme
et servent de référence à sa qualification.

## Qualification du mécanisme intégré

Le pilote `e74a86d` a reconnu l’annonce MSI résiduelle dans la VM x64 et
a préparé la récupération à partir du MSI officiel v0.21.0 déjà vérifié.
Le même pilote a refusé une récupération superflue après la panne avant
retrait, un MSI absent et un MSI d’une autre version. Le refus UAC lors de
la réinstallation interactive a renvoyé `1602`. Le journal du pilote
contient `msi_cancelled` et une observation suivante confirme la
conservation des données de test.

Une seconde tentative interactive a rétabli le ProductCode sans rétablir
l’index. Le retrait explicite a rendu `0` et l’observateur indépendant a
confirmé l’absence des deux ProductCode. Après fermeture du terminal et
relance du pilote, la réinstallation a rétabli le ProductCode, le cache
et l’index dans le compte standard. La mise à niveau vers le MSI hybride
`e74a86d` a ensuite validé les 16 fichiers, l’IPC 10, la fermeture de
Flutter avec le tray encore actif et la désinstallation. Les données
de test et l’état d’autostart ont été conservés.

L’échec injecté après retrait du bundle `e74a86d` a également produit
`1603`. L’observateur a relevé l’absence des deux ProductCode et de
l’index, tandis que les 16 fichiers ont conservé leurs empreintes.
Pour ce scénario hors ligne, la copie de test du pilote utilise
l’empreinte du paquet GitHub Actions authentifié. La compilation
diffusée conserve la vérification de provenance de la release.
Dans la VM, le pilote a refusé un lancement sous jeton élevé. Un
mutex MSI occupé artificiellement dans la VM a confirmé le refus de
récupération, en simulation d’une transaction concurrente.

Après une réinstallation interactive avec UAC, le ProductCode du
bundle v0.22.0 était enregistré dans le compte standard et restait
introuvable via l’UpgradeCode. Le retrait confirmé l’a rendu absent.
Le redémarrage de Windows dans la VM a eu lieu après ce retrait.
Le pilote a retrouvé le journal et proposé l’installation propre.
L’annulation à ce stade a produit `cancelled_before_msi` sans lancer
Windows Installer. La tentative suivante a rétabli le ProductCode,
le cache, l’index UpgradeCode et les 16 fichiers attendus.
La désinstallation du bundle rétabli a réussi et les données
de test sont restées identiques.

Le mécanisme compense la perte d’enregistrement constatée dans le
compte standard. Pour la récupération du bundle v0.22.0, la VM hors
ligne a utilisé le paquet GitHub Actions authentifié et l’empreinte
intégrée à la copie de test. La récupération diffusée utilise la
provenance de la release et laisse l’intervention en attente lorsque
celle-ci ne peut pas être vérifiée.

Le MSI hybride `e74a86d` de qualification Windows a pour SHA-256
`8355bdf7e36aa395b5f4c379d4020cc3daa8f5e6fb5403f08b846f78f885ed83`.
Le [rejeu administrateur du même MSI](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45378171795/job/138389905265)
valide la mise à niveau, les rollbacks du scénario administrateur,
l’IPC 10 et le retrait. Le test attend l’accusé de réception IPC du PID
Flutter observé avant de fermer sa fenêtre. Le
[rejeu en compte standard hébergé](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45378171795/job/138389904643)
est bloqué dès l’installation de v0.21.0 par `DisableMsi=1` et `1625`.

Les [gardes natifs ciblés](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45363900624)
ont réussi sur Linux x64 et ARM64 et macOS Intel et Apple Silicon à
`f7e3d87`. Les corrections suivantes concernent Windows et son banc de
test.
