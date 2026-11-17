# Distribution hybride MicCamWatch 0.22.0

Les paquets de qualification gardent les noms `miccamwatch-hybrid-…`. Le
client Windows 0.21.0 télécharge le ZIP natif historique et en extrait seulement
`mcw.exe` et `mcw-tray.exe`. Le nom distinct évite que ce client choisisse
un paquet hybride pour une opération qui ne couvre que les deux exécutables.

## Contenu livré

| Cible | Artefacts de qualification | Emplacements dans le paquet |
|---|---|---|
| Windows x64 | MSI par utilisateur et ZIP | `mcw.exe`, `mcw-tray.exe`, `miccamwatch-bundle.json`, `desktop/miccamwatch_desktop.exe`, `desktop/flutter_windows.dll`, `desktop/data/app.so`, `desktop/data/icudtl.dat`, `desktop/data/flutter_assets/`, `README.md` et `LICENSE`. Le MSI conserve `%LOCALAPPDATA%\Programs\MicCamWatch`, l’identité d’upgrade, le raccourci et le PATH utilisateur. |
| Linux x64 / ARM64 | TAR.GZ par architecture | `mcw` héberge le tray SNI. `desktop/miccamwatch_desktop`, `desktop/lib/{libapp.so,libflutter_linux_gtk.so}`, `desktop/data/{icudtl.dat,flutter_assets/}`. Le helper `mcw-camera-helper`, son installateur et la politique Polkit restent à la racine avec le manifeste et les notices. |
| macOS Intel / Apple Silicon | DMG et TAR.GZ par architecture | `MicCamWatch.app/Contents/MacOS/{MicCamWatch,mcw}`. `Contents/Helpers/MicCamWatch.app` contient le runner et ses frameworks `App.framework` et `FlutterMacOS.framework` avec AOT, ICU, assets et signature. Le manifeste, l’icône et les notices résident sous `Contents/Resources`. |

Le [contrat exact Windows et Linux](../installer/hybrid_layout.json) énumère
les fichiers. Les sept langues sont compilées avec les vues Flutter. Rust
continue à lire les préférences, les politiques et l’historique dans les
emplacements natifs. Les pseudo-fichiers de test et les builds instrumentés
sont en dehors du paquet. La création des archives refuse les chemins et les
fichiers inattendus, et relit le contenu effectif des conteneurs.

## Opérations

**Windows.** Le MSI effectue l’upgrade du produit par utilisateur. Le retrait
de l’ancien produit intervient après l’ouverture de la transaction MSI, ce
qui permet à Windows Installer de le rétablir en cas d’échec. Pour le retrait
manuel, fermer la fenêtre, arrêter le tray par la commande native, traiter
explicitement les protections détenues et couper l’autostart appartenant à
l’installation avant le retrait via Paramètres. Le ZIP fournit une
installation portable complète dans un répertoire distinct. Le client Rust
de mise à jour historique reste écarté des transactions hybrides.

**Linux.** Le script [d’installation](../installer/hybrid_linux_install.py)
prend un TAR.GZ et l’empreinte SHA-256 vérifiée indépendamment. Choisir un
préfixe dédié avec un parent détenu par l’utilisateur. L’exécutable actif
reste `<préfixe>/bundle/mcw`. Un emplacement stable conserve les chemins
du tray et de l’autostart. Le verrou partagé de Rust exclut un échange
pendant qu’une instance du bundle est en service. Après staging vérifié,
`renameat2` échange deux répertoires entiers. Le script retourne le slot
exact pour le retour arrière. `mcw uninstall` vérifie l’intégrité, le
propriétaire, la protection automatique et les restaurations possédées avant
de retirer le bundle et son autostart. Les données de profil et les paquets
de récupération restent dans leurs emplacements respectifs. L’installation
du helper privilégié relève de son installateur dédié et de Polkit.

```sh
mkdir -p "$HOME/.local/opt"
python3 installer/hybrid_linux_install.py --root "$HOME/.local/opt/miccamwatch" install "$archive" --sha256 "$sha256"
python3 installer/hybrid_linux_install.py --root "$HOME/.local/opt/miccamwatch" rollback "$rollback_slot"
"$HOME/.local/opt/miccamwatch/bundle/mcw" uninstall
```

**macOS.** Installer ou échanger le fichier .app entier depuis l’image
vérifiée, une fois le propriétaire Rust arrêté. Conserver la copie précédente
jusqu’à la vérification de l’installation. Les signatures ad hoc du
lanceur, de l’app Flutter et des frameworks sont relues après emballage.
Une diffusion Developer ID devra signer les frameworks et exécutables
imbriqués avant l’app externe, avec les entitlements contrôlés, puis soumettre
l’image à la notarisation, agrafer le ticket et vérifier Gatekeeper. Ces
opérations dépendent des identifiants de distribution.

Les validations de périphériques physiques et d’autorisations UAC, Polkit et
TCC appartiennent à la qualification ultérieure. Pour chaque essai, relever
l’identité exacte du périphérique, l’autorité du compte et les états avant
l’action. Vérifier ensuite le refus ou l’effet réel, la restauration des
états d’origine et l’intégrité après fermeture du frontend.

## Scripts de construction et de qualification

- Windows utilise [hybrid_windows.py](../installer/hybrid_windows.py), le
  [modèle WiX](../installer/miccamwatch.wxs) et les essais
  [MSI](../installer/tests/windows-hybrid-install.ps1) et
  [compte standard](../installer/tests/windows-hybrid-standard.ps1).
- Windows et Linux utilisent [hybrid_payload.py](../installer/hybrid_payload.py)
  pour assembler et relire les archives. Linux ajoute
  [l’audit ELF](../installer/hybrid_linux_abi.py), [l’installateur et son rollback](../installer/hybrid_linux_install.py)
  et [le retrait natif du bundle](../src/uninstall/linux_bundle.rs).
- macOS utilise [son assembleur](../installer/macos/package.py) et
  [son essai installé](../installer/tests/macos-hybrid-installed.py).
- La vérification de cohérence Windows/Linux avant ouverture de Flutter
  repose sur [le contrôle Rust](../src/frontends/desktop/package.rs).
  Les builds et essais sont pilotés par la
  [qualification Windows](../.github/workflows/flutter-windows-qualification.yml)
  et la [qualification Linux et macOS](../.github/workflows/flutter-qualification.yml).

## Qualifications de l’étape 3

Les paquets portent la version 0.22.0 et l’IPC 10. Le manifeste
Windows/Linux fixe la cible, la révision et l’empreinte de chaque fichier.
Rust refuse un fichier indispensable manquant ou modifié et une version,
architecture ou révision d’IPC incompatible avant l’authentification du
frontend. Les contrôles macOS relisent le bundle et les signatures imbriquées.

- **Windows x64.** Builds Rust et Flutter release, audit des imports PE
  ordinaires et différés, extraction intégrale du MSI et du ZIP sur
  `c36c68498f46b1c7abaac22898efd8bd708325ef`. L’audit des imports recouvre
  le runner, la DLL Flutter, le CLI et le tray. Windows Installer conserve
  l’UpgradeCode historique et l’installation par utilisateur.
  [Job de construction et d’audit](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44033442200/job/133870562204).
  Sur le runner administrateur, le MSI officiel v0.21.0, la mise à niveau
  réelle et les deux rollbacks injectés ont été exercés. Le premier restaure
  le produit v0.21.0. Le second restaure tous les fichiers et le manifeste de
  la v0.22.0 hybride après le retrait du produit par un MSI de test d’une
  version supérieure. Le Flutter installé a reçu le premier état par IPC,
  a affiché sa fenêtre et l’a refermée avec le même tray en service.
  Après arrêt coopératif, le retrait MSI a supprimé la charge utile et le
  raccourci et conservé les préférences et l’historique. La fixture avait
  créé le verrou natif `.mcw-install.lock` en activant l’autostart de
  v0.21.0. Ce verrou appartient au runtime et reste dans le répertoire
  après le retrait. Le ZIP a été comparé au manifeste du paquet, y compris
  les octets du tray et sa sortie de version. Son cycle de maintenance
  n’est pas qualifié.
  La révision des scripts de qualification est
  `a4fa992a973c9d5cce95b425c471580090cc7b87`.
  [Cycle MSI administrateur](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44069702318/job/133992499050),
  [essai en compte standard](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44069702318/job/133992498770).
- **Linux x64 et ARM64.** TAR.GZ construits sur deux hôtes natifs Ubuntu 22.04
  depuis `9f4963f88d032ee5f7804475bcdb60c24f0abb1f`. Les reçus
  `elf-abi.json` contrôlent cinq ELF par cible, l’architecture et la
  résolution des dépendances sur glibc 2.35, avec des plafonds GLIBC 2.35,
  GLIBCXX 3.4.30 et CXXABI 1.3.13. Sur les hôtes, le contrôle de
  désinstallation a détecté les chemins système sous `/usr/share`. Les deux
  essais installés ont ensuite réussi sous compte ordinaire dans des
  conteneurs Ubuntu 22.04, avec les mêmes gardes Rust. Ils ont vérifié le CLI
  0.22.0 issu de l’archive, les refus sur état de protection et verrou détenu,
  le retrait du bundle, celui de son autostart et la conservation des données.
  Trois essais sur des arbres de test vérifient l’upgrade, le retour arrière
  par échange atomique et une interruption avant échange. La fenêtre X11 et
  le tray SNI x64 ont été exercés avec le bundle de développement de cette
  révision. Les essais installés étaient sans fenêtre sur les deux cibles.
  L’exécution graphique ARM64 reste ouverte.
  [x64](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44033985419/job/133872329758),
  [ARM64](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44033985419/job/133872330130).
- **macOS Intel et Apple Silicon.** DMG et TAR.GZ construits sur
  `14cbf40406ae24cee63a0df919393f394cfae1cb`. Les deux DMG ont servi à la
  mise à niveau depuis l’app officielle v0.21.0, au refus d’un sceau
  altéré, au rollback et à la suppression de l’app entière. Les deux
  lancements installés par architecture ont reçu l’acquittement IPC de Rust.
  L’arrêt coopératif a libéré les processus et les octets des préférences
  et de l’historique correspondent au relevé avant installation. Le Quit
  graphique a été vérifié dans un autre essai. Les signatures ad hoc sont
  vérifiées. Developer ID et notarisation restent à effectuer avec les
  identités de distribution.
  [Intel](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44030833633/job/133862116480),
  [Apple Silicon](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44030833633/job/133862117116).

### Empreintes SHA-256

Les empreintes ont été recalculées sur les huit fichiers téléchargés.
Chaque jeu conserve la révision de construction indiquée ci-dessus.
Les jobs d’installation Windows ont repris l’artefact MSI d’origine après
vérification de son empreinte et de sa provenance.

```text
612a24a4cbf9a7206f5b6f972fc6a4b5fbb8179d1d51a7426c865579817bf25e  miccamwatch-hybrid-windows-x86_64.msi
5f96a2f3f426b7b4945504c4bc249b0ee08e70908bc7ebf4839b6cd135c7f224  miccamwatch-hybrid-windows-x86_64.zip
d9ad9f2bd17f31e3b76f68b004d6020bc686c08395d0f24b8c47ecc3bfa4e5e8  miccamwatch-hybrid-linux-x86_64.tar.gz
be290f2ea469444a7679d48772758f427431ab9657a7300a77584e63fa3a593f  miccamwatch-hybrid-linux-aarch64.tar.gz
e1a21e683f9e3af66a2c4e50b6257738e8feb3d673e43760dde208bb22b9b568  miccamwatch-hybrid-macos-x86_64.dmg
b0c189545b83febb0f944c21384a4155e599e222f1591594f986350be789f34b  miccamwatch-hybrid-macos-x86_64.tar.gz
2a8e1c6e7c7e2a297b2427df208c419d9a313e57b04f9ad8ee7bac11802ee799  miccamwatch-hybrid-macos-aarch64.dmg
c215e5bd7003c88b7acb2e40ebdbae98522fb75ce446c868d04c8777b3cabf3e  miccamwatch-hybrid-macos-aarch64.tar.gz
```

## Maintenance Windows et reprise

**MSI.** `mcw update` relit l’enregistrement Windows Installer du produit
par utilisateur, son emplacement et le MSI en cache. Il vérifie
l’UpgradeCode, le ProductCode, la version, le type d’installation et la
compatibilité IPC avant de transférer la mise à niveau à Windows Installer.
La découverte utilise la release officielle du dépôt GitHub. L’empreinte SHA-256
de l’artefact renvoyé par son API HTTPS doit correspondre à l’entrée de
`SHA256SUMS` et aux octets téléchargés. Ce contrôle authentifie l’origine
via GitHub. La signature Authenticode de diffusion reste à effectuer.

Le terminal demande de saisir `update` pour confirmer. Les API natives de
maintenance chargent leurs DLL depuis System32. Le client exige le profil
Windows réel, la sortie normale du tray et les réservations
natives qui protègent le micro et la caméra. Il attend le résultat de
Windows Installer dans un processus distinct dont l’identité est vérifiée.
Un code de sortie zéro est suivi d’une relecture du nouveau ProductCode,
de la disparition de l’ancien, du manifeste, des binaires et des versions
Rust et Flutter. Les refus, les redémarrages requis et les états non
vérifiables restent distincts dans le reçu. Une interruption sans code de
sortie ou un redémarrage demandé impose de redémarrer Windows avant de
revérifier l’installation.

**ZIP portable.** Extraire le paquet vérifié dans un dossier utilisateur
dédié sous `%LOCALAPPDATA%`, hors du chemin MSI. Lancer la commande depuis
un autre dossier du terminal. L’extraction vérifie la liste exacte des entrées
et les empreintes du manifeste. Les répertoires, les liens, les doublons,
les entrées inattendues, les architectures incompatibles et les versions
ou protocoles IPC mélangés entraînent un refus. La protection de l’installation
empêche le lancement de MicCamWatch pendant les deux renommages. Le journal
est écrit avant la bascule et le bundle précédent est retenu entier jusqu’au
contrôle final. Les nouveaux objets de maintenance reçoivent le SID exact
du compte, y compris lorsque le jeton est élevé.

Pour retirer le ZIP, désactiver l’autostart de cette installation avec
`mcw autostart disable`, quitter normalement le tray puis lancer
`mcw uninstall` et saisir `uninstall` dans le terminal. Les données de profil
restent dans leur emplacement usuel. Le retrait de l’instance active précède
le nettoyage de sa copie vérifiée. Le reçu indique si une copie a dû être
conservée.

Le client indique le chemin de `result.json` après avoir transmis la
demande. Le reçu est créé après l’opération. Un dossier
`.<nom-du-bundle>.mcw-maintenance\update-…` reste à côté de l’installation.
Pour une transaction inachevée, exécuter le `maintenance.exe` indiqué dans
le diagnostic avec `recover-update`. Les fichiers `committed` et `rolled-back`
consignent l’issue de la reprise. Le reçu JSON décrit la tentative initiale.
Le rollback ZIP relit les arborescences
avant de rétablir l’ancienne. En présence d’une arborescence inconnue ou d’un
enregistrement MSI impossible à vérifier, la récupération conserve les
fichiers et l’état incomplet pour inspection. Le CLI ne relâche aucune
protection active pour poursuivre l’opération.

### Qualification de la maintenance

- Windows natif, Clippy strict avec et sans le profil empaqueté. Les 31 tests de
  maintenance couvrent les archives et ressources invalides, les refus de
  protection, les renommages complets, les interruptions simulées entre les
  renommages, le rollback et le retrait. Le test de métadonnées a refusé des copies du MSI avec architecture,
  UpgradeCode ou ProductCode falsifiés et un fichier MSI corrompu. Il a lu un MSI
  produit par WiX en réutilisant la charge utile historique, sans l’installer.
  La fixture WiX SHA-256 est
  `edbb329f0392387cc0f1e2e39f6569e4dc09427ac8428b640cf536fe39df385e`.
- Le test de panne injectée du MSI historique sur ARM64 a restauré les exécutables
  de v0.21.0. Les erreurs 1401/1406 et le code Windows 5 dans le registre
  Windows Installer ont empêché de valider les métadonnées rétablies.
  Le parcours x64 standard précédent reste bloqué par `DisableMsi=1`
  et le code 1625. Aucun runner dédié n’est enregistré et l’accès
  Hyper-V local a été refusé. La procédure reproductible pour un
  Windows x64 jetable figure dans le [guide compte standard](WINDOWS_STANDARD_USER.md).
  [Rollback ARM64](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44122044173/job/134175745899),
  [règles Windows Installer](https://learn.microsoft.com/en-us/windows/win32/msi/disablemsi).
- Le nouveau MSI x64 et le ZIP ont été construits depuis
  `7e5c1fff1e90a7714f4e2c37d3900fdfa5a3864a`, avec les octets Flutter audités
  de `c36c68498f46b1c7abaac22898efd8bd708325ef` et un arbre Flutter identique.
  Le job x64 a validé les imports, l’extraction des deux formats, les
  31 tests Rust, la lecture du ZIP et de la version Flutter, puis la
  délégation réelle par Rust à Windows Installer depuis v0.21.0. Les deux
  rollbacks injectés en administrateur, l’acquittement IPC 10, la poursuite
  du tray après fermeture de Flutter et la désinstallation avec données
  conservées ont réussi. Le MSI de qualification est dépourvu de signature
  Authenticode. [Construction et installation x64](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44131699871/job/134210242949).
- Le même MSI x64 a réussi le cycle d’installation v0.21.0, upgrade v0.22.0
  et retrait sur Windows 11 ARM64 en émulation, avec un vrai compte standard.
  Le reçu confirme les fichiers, l’autostart et la conservation des données.
  Le job a exercé le parcours normal en mode non interactif. Les scénarios
  de panne, l’ouverture de la fenêtre et les dialogues UAC relèvent d’autres
  essais. [Compte standard, nouveau MSI](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44133319555/job/134216026689),
  [rejeu historique](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44124122154/job/134183131331).

Les empreintes suivantes ont été recalculées sur les nouveaux artefacts
téléchargés et comparées au `SHA256SUMS` du job.

```text
2d5b508c08baf660bbcf8a27a7ab601b4ec6c5185314666502f9bca16540a110  miccamwatch-hybrid-windows-x86_64.msi
556c776b064cf324a447be502781d8ac18ee260a7afc8d0925c7ddf696ddf049  miccamwatch-hybrid-windows-x86_64.zip
```
- Les gardes natifs Linux x64 et ARM64, macOS Intel et Apple Silicon ont
  exécuté les deux cas isolés attendus par cible sur
  `2cb679a9ac96271cfbe8ef5f2b07e97a12c1342a`. Les modifications suivantes
  concernent le chargement des DLL Windows et son workflow. Les preuves de
  cycle installé, ABI et signatures Unix ci-dessus gardent leurs sources.
  [CI gardes Unix](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44124306625).

**ÉTAPE 3/6 : INCOMPLÈTE.** Le rollback en compte standard a un défaut de
restauration de l’enregistrement à investiguer. La qualification interactive
Windows x64, UAC et le téléchargement d’un vrai successeur depuis une release
officielle restent ouverts.
