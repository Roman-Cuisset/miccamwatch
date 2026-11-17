# Distribution hybride MicCamWatch 0.22.0

Les paquets de qualification gardent les noms `miccamwatch-hybrid-…`. Le
client Windows 0.21.0 télécharge le ZIP natif historique et en extrait seulement
`mcw.exe` et `mcw-tray.exe`. Le nom distinct évite que ce client choisisse
un paquet hybride pour une opération qui ne couvre que les deux exécutables.

## Contenu livré

| Cible | Conteneurs privés | Emplacements dans le paquet |
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
