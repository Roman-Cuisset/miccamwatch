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
