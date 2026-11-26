# MicCamWatch v0.22.0 — qualification de la candidate

**ÉTAPE 5/6 : INCOMPLÈTE.** Qualification en cours sur
`work/flutter-hybrid-0220`. Révision de construction
`de90a76311445ba6607e6f3df4fce988a1ffc343`. Le compte standard de la VM est un
critère séparé et doit utiliser le MSI exact de cette révision.
[CI 45835303021](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45835303021).

Le HEAD documentaire de l’étape 4 était `a6d5d83b03e2315fd62d27c9f34349b6f36220f7`.
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
du lot final sont contrôlés séparément. Les huit paquets de la CI source ont
été téléchargés et leurs empreintes comparées aux manifestes de plateforme.
Le MSI, les trois EXE Windows et `flutter_windows.dll` sont `NotSigned`.
Les deux apps macOS ont passé la relecture de leurs scellés ad hoc et de
leurs conteneurs sur les runners natifs.

```text
184b0510569fced919c8cfd62f94f34c831bcfa309136ebe51ee9d4281d74d2a  miccamwatch-hybrid-windows-x86_64.msi
6917a95ff09832f0b95822ac92fe2f6ee041790aec431d11f7dfe8c4efd952bc  miccamwatch-hybrid-windows-x86_64.zip
7f8e7dd5be086c99521f4556095be73b9157e472d8e42da6160272fe070be253  miccamwatch-hybrid-linux-x86_64.tar.gz
da434485889fc5631f69a95fa8aa9275378ad6d3368a68ec59b6c9dfa9e54cc1  miccamwatch-hybrid-linux-aarch64.tar.gz
18099c4ebdb64dd41fb818f47cacec7b77d74743a7ad8351ecdf6f3a51c798f0  miccamwatch-hybrid-macos-x86_64.dmg
44c56509fbe384da3fbd3db791f589c44dd294f5f3d00b165d4f9edffb86ffc9  miccamwatch-hybrid-macos-x86_64.tar.gz
2037f4ae3e56a8aa1f904cc2219c22f8e96ceb5b12e42bac67e52bf2f5a12556  miccamwatch-hybrid-macos-aarch64.dmg
16b1b74030a02d6ae4c042c2ef2c1823c936ac1b6b5ec245da77cc2fe2c2f26f  miccamwatch-hybrid-macos-aarch64.tar.gz
```

## État des contrôles

- Formatage Rust et audit de `Cargo.lock` réussis. Les quatre avertissements
  de maintenance `chrono`, `zbus_names`, `zvariant` et `zvariant_derive`
  restent visibles. Preuves sous `target/stage5-qualification/45835303021`.
- Les quatre jobs Unix de la CI source sont réussis. Les deux installations
  Linux utilisent les archives exactes dans des conteneurs Ubuntu 22.04
  natifs. Elles vérifient les refus de protection et de bail exclusif, le
  retrait de l’autostart et la conservation des données. Le test d’échange
  et de rollback utilise des bundles de test distincts. Les deux DMG ont
  passé le cycle d’installation et de rollback d’application. Les reçus
  macOS indiquent que les bannières n’ont pas été observées.
- Le MSI Windows a réussi dans le runner hébergé, dont le scénario de
  rollback et les 34 tests Rust de maintenance empaquetée. Le run source
  reste en échec car le test de démarrage exige un runner vierge et a
  refusé le dossier laissé après le scénario MSI. Une reprise sur un
  runner séparé réutilise les bundles Flutter du run source et compile
  uniquement le moteur de test. [Reprise Windows](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45840695609).
- Sur Linux ARM64, le premier essai de bureau a révélé l’absence de
  `libGLESv2.so.2` sur le runner minimal. Après installation de `libgles2`,
  deux fenêtres possédées ont été ouvertes et fermées avec IPC prêt, puis
  le tray est resté actif. La CI a ensuite qualifié ce parcours séparément
  du retrait déjà contrôlé en conteneur. Le rendu est celui de Xvfb et ne
  prouve pas l’accélération sur matériel réel.
  [Preuve ARM64](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45840703349).

Le passage de la candidate exacte sous compte standard dans `MCW-Win11-x64-Lab`
est **réussi**. Les trois transactions MSI ont rendu le code 0. L’observateur
administrateur distinct a confirmé le ProductCode
`{AD7C13A4-524E-4667-A0E9-66066FE9346C}`, l’UpgradeCode
`{8FC9317B-4517-4E83-9F52-CF3257584BF6}`, la version, le cache et le retrait
de l’inscription. L’IPC 10, l’intégrité du payload, l’arrêt du frontend avec
le tray encore actif et la conservation des données ont été vérifiés.
Les reçus et journaux sont dans
`target/stage4-qualification/lab-consolidated-20261126-142901`.
La tâche avait attendu l’alimentation secteur. Le diagnostic a relevé la
condition batterie, le bureau QA actif et aucun client MSI concurrent. Le
branchement au secteur a permis la reprise avec les mêmes réglages.

L’étape 4 a conservé des preuves logicielles pour l’IPC, les sept langues,
les deux thèmes, l’historique, les préférences, le démarrage et l’autostart,
le cycle Flutter et les limites des collecteurs. Les gardes de récupération
et l’ensemble des pannes historiques restent attachés aux révisions réellement
exercées. Le passage positif en VM couvre le nouveau MSI. La politique
`DisableMsi=1` du runner standard hébergé reste intacte.

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
