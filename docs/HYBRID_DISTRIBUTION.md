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

### Points ouverts

- `mcw update` refuse la transaction historique sur un bundle hybride.
  Windows Installer gère l’ensemble du MSI. Le transfert du client Rust
  vers un MSI vérifié et le cycle de mise à jour, retrait et récupération
  après interruption du ZIP portable restent à implémenter. Le rollback
  du MSI s’applique pendant la transaction. Un retour à une version
  antérieure après validation relève d’une autre procédure.
- Le MSI de qualification est dépourvu de signature Authenticode. Le jeton
  et le profil d’un véritable compte local standard ont été vérifiés.
  La politique Windows Installer du runner (`DisableMsi=1`) refuse le
  MSI de départ avec le statut 1625. Un bureau Windows jetable qui autorise
  normalement le MSI par utilisateur reste requis pour les essais MSI
  interactifs en compte standard et les dialogues UAC.
- La qualification ultérieure doit couvrir les périphériques physiques
  et les autorisations UAC, Polkit et TCC en environnement isolé. Vérifier
  les signatures des paquets destinés à la diffusion sous leurs identités
  de distribution.

**ÉTAPE 3/6 : INCOMPLÈTE.** La maintenance Rust du bundle Windows et la
récupération du ZIP portable restent ouvertes.
