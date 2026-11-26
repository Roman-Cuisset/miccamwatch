# MicCamWatch v0.22.0 — qualification de la candidate

**ÉTAPE 5/6 : INCOMPLÈTE.** Construction et qualification en cours sur
`work/flutter-hybrid-0220`. Révision de construction
`de90a76311445ba6607e6f3df4fce988a1ffc343`. Le compte standard de la VM est un
critère séparé et doit utiliser le MSI exact de cette révision.
[CI 45835303021](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45835303021).

La source de l’étape 4 était `a6d5d83b03e2315fd62d27c9f34349b6f36220f7`.
La branche intègre la préparation de l’assemblage RC et le README distribué.
Rust, Dart, Swift, le code WiX, les formats de bundle et les verrous de
dépendances du produit sont identiques à ceux du HEAD de l’étape 4.
Les huit paquets d’étape 4 ont été revérifiés par SHA-256. Le collecteur
les a acceptés sous leur vraie source `2aae176` et a refusé un MSI corrompu
dans une copie de test. Ces octets historiques gardent leur provenance.
[Transfert de l’étape 4](HYBRID_STAGE4_CODEX_HANDOFF.md).

## Matrice et livrables

| Cible | Paquets attendus | Contrôles de la candidate |
| --- | --- | --- |
| Windows x64 | `miccamwatch-hybrid-windows-x86_64.msi`, `miccamwatch-hybrid-windows-x86_64.zip` | CI hybride, inspection MSI et ZIP, mise à niveau et récupération en CI, puis installation et inscription MSI observées dans la VM avec le compte standard |
| Linux x64 | `miccamwatch-hybrid-linux-x86_64.tar.gz` | Rust et Flutter Release, ABI, exécution hébergée, installation gérée, rollback et retrait |
| Linux ARM64 | `miccamwatch-hybrid-linux-aarch64.tar.gz` | Build natif depuis le SDK Flutter officiel amorcé sur ARM64, ABI, installation gérée, rollback et retrait |
| macOS Intel | `miccamwatch-hybrid-macos-x86_64.dmg`, `miccamwatch-hybrid-macos-x86_64.tar.gz` | AppKit, Flutter, scellés, architectures, application installée et rollback |
| macOS Apple Silicon | `miccamwatch-hybrid-macos-aarch64.dmg`, `miccamwatch-hybrid-macos-aarch64.tar.gz` | Même parcours sur runner ARM64 natif |

Cargo et Flutter indiquent `0.22.0` et le protocole IPC est `10` des deux
côtés. Les workflows épinglent Flutter 3.47.8 et Dart 3.13.7. Les builds
de qualification instrumentés restent dans leurs artefacts de preuve.
Les paquets candidats passent le contrôle du contenu et de la révision
par les constructeurs natifs, puis celui de leurs empreintes à l’assemblage.
L’ensemble prévu comprend huit paquets, le SBOM SPDX et `SHA256SUMS`.
Les SHA-256, la couverture du SBOM, les signatures et les neuf attestations
restent à relever et à vérifier après construction.

L’étape 4 a conservé des preuves logicielles pour l’IPC, les sept langues,
les deux thèmes, l’historique, les préférences, le démarrage et l’autostart,
le cycle Flutter et les limites des collecteurs. Les gardes de récupération
et l’ensemble des pannes historiques restent attachés aux révisions réellement
exercées. Le passage positif MSI de la candidate requiert une nouvelle mesure
dans la VM à cause du changement d’octets. La politique `DisableMsi=1` du
runner standard hébergé reste intacte.

## Limites acceptées

Les essais physiques manquants ne sont pas des conditions de publication.
Le rapport précédent décrit précisément Vorssaint réel, la caméra Mac après
veille, les bannières et autorisations persistantes, les lecteurs d’écran,
le retrait physique d’un écran, le GPU Linux et les mesures en capture réelle.
Un défaut critique établi de sécurité ou de fonctionnement reste bloquant.
[Preuves et risques résiduels](HYBRID_STAGE4_CODEX_HANDOFF.md#fonctions-prouvées-limites-acceptées-et-risques-résiduels).

## Transfert pour la publication

Les [notes anglaises](RELEASE_0.22.0_NOTES.md), le
[changelog](../CHANGELOG.md), les
[instructions d’installation](INSTALLATION.md) et le
[brouillon de diffusion](RELEASE_0.22.0_PUBLICATION_DRAFT.md) servent à préparer
la publication lors d’une mission séparée. Le brouillon contient les sept
langues du site et les champs WinGet à renseigner à partir des octets publics.

## Stockage et conservation

Le disque C: disposait de **46,65 Gio** libres avant le lancement de la CI.
Les mesures ci-dessous sont des tailles logiques observées avant les nouveaux
téléchargements. Aucun fichier n’est supprimé par cette mission.

| Emplacement | Taille | Proposition après publication et validation |
| --- | ---: | --- |
| `miccamwatch/target/debug/deps` et `target/debug/build` | 16,77 Gio | Résultats régénérables. Conserver les journaux et vérifier l’absence de compilation active avant nettoyage ciblé. |
| `miccamwatch/target/release/deps` | 2,58 Gio | Résultats régénérables, distincts des paquets validés. |
| `mcw-flutter-0220/target/debug` et `target/release` | 4,95 Gio | Résultats régénérables. Vérifier l’archivage des binaires encore référencés. |
| `target/mcw-hyperv-lab-inventory/recovery-build` | 4,77 Gio | Examiner les dépendances des reçus avant de supprimer les résultats de compilation. |
| `target/stage4-qualification` | 1,46 Gio | Conserver reçus source et preuves uniques. Envisager uniquement des doublons dont l’empreinte et la copie d’archive ont été vérifiées. |
| ISO officielle et fichier partiel associé | 8,43 Gio chacun | Comparer les octets et vérifier les médias attachés avant de proposer le retrait du fichier partiel. Garder l’ISO nécessaire. |
| Disques Hyper-V du laboratoire | 46,45 Gio environ | Conserver la chaîne VHDX et AVHDX. Toute rétention ou fusion de checkpoint se décide dans Hyper-V après sauvegarde et accord distinct. |

Les quatre premiers ensembles représentent environ **29,07 Gio logiques**
potentiellement récupérables, sous réserve des vérifications ci-dessus et
de l’espace réellement alloué. Les 14 worktrees et les preuves MSI sont à conserver.
