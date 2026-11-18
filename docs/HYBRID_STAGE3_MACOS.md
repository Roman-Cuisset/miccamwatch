# MicCamWatch v0.22.0 sur macOS

## Démarrage automatique et chemin de l’application

Le message rapporté sur un vrai Mac équipé de v0.21.0 provient du contrôle
du chemin de l’exécutable avant l’enregistrement du LaunchAgent. Le garde
ouvrait chaque dossier sans suivre de lien, mais affichait le chemin complet
même si le contrôle avait échoué plus haut. Une autorisation d’écriture de
groupe sur `/Applications`, sur le bundle ou sur l’un de ses dossiers internes
explique ce message. Le chemin tronqué de la remontée ne permet pas de
distinguer ces cas et les permissions du Mac concerné restent à relever.
Les runners Intel et Apple Silicon présentent un `/Applications` sans lien
symbolique, détenu par `root:admin` (UID 0, GID 80), de mode `0775`. Ce mode
permet à un membre du groupe d’écrire dans le dossier et suffit à
déclencher le contrôle avant l’inspection du bundle.

La v0.22.0 désigne le dossier qui a provoqué le refus et donne son UID,
son GID et son mode. Le contrôle existant de la propriété, des permissions
et des liens reste en vigueur. Le chemin physique de l’exécutable est
enregistré après contrôle des ACL Darwin sur les descripteurs ouverts. Une
ACL qui accorde des droits de modification est refusée. Une ACL de lecture
ou de refus reste compatible avec le contrôle. Le diagnostic ne change
aucune permission.

`/Applications` est le domaine local partagé. `~/Applications` est le
domaine du compte. Une installation dans le dossier utilisateur est compatible
avec l’autostart lorsque le compte possède l’exécutable et que les dossiers
du chemin satisfont les mêmes contrôles. Une copie système protégée peut
être lancée manuellement et rester inéligible à l’autostart utilisateur selon
le propriétaire et les permissions. [Domaines du système de fichiers Apple](https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/FileSystemOverview/FileSystemOverview.html).

Sur le Mac du testeur, relever le message complet et lancer ce diagnostic dans
Terminal, sous le compte habituel. Ajuster `app` au dossier réellement ouvert.
Le chemin personnel peut être masqué dans les lignes transmises en conservant
les noms des dossiers de l’application, les UID/GID, les modes et les types
d’entrées ACL.

```sh
app='/Applications/MicCamWatch.app'
bin="$app/Contents/MacOS/mcw"
/usr/bin/sw_vers -productVersion
/usr/bin/id -u
/usr/bin/id -G
inspect_path() {
    item="$1"
    while :; do
        /usr/bin/stat -f '%N | uid=%u gid=%g mode=%OLp type=%HT' "$item"
        /bin/ls -ldeO "$item"
        [ "$item" = / ] && break
        item="$(/usr/bin/dirname "$item")"
    done
}
inspect_path "$bin"
physical="$(/usr/bin/realpath "$bin")" && inspect_path "$physical"
/usr/bin/codesign --verify --deep --strict --verbose=2 "$app"
```

Le diagnostic lit le chemin apparent, puis le chemin physique. Les preuves
attendues sont le dossier exact, la possibilité de remplacer l’entrée
suivante (mode ou ACL), la propriété de l’exécutable et la présence éventuelle
d’un lien. Si `realpath` est indisponible, les résultats du premier parcours
et les cibles affichées par `ls` restent exploitables. Les essais automatisés
couvrent des répertoires privés, des répertoires modifiables par le groupe ou
par tous, des liens et des ACL de lecture et de modification.
Sur le code `18feb1aed8439447fac580a49fd41168988bdd07`, les deux Mac hébergés
ont réussi chacun neuf tests d’autostart, dont le refus de l’écriture via
ACL sur un fichier et un dossier. Les deux Linux ont réussi chacun dix
tests d’autostart. Clippy strict et les deux contrôles du menu natif ont
réussi sur les quatre cibles. [Preuves par architecture](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44245431361).

## Identité des notifications et des capteurs

Le bundle extérieur utilise `com.roman-cuisset.miccamwatch`. L’interface
Flutter sandboxée utilise `dev.miccamwatch.desktop`. Le processus Swift
emploie `com.roman-cuisset.miccamwatch.helper` dans son Info.plist, sa
signature ad hoc et la vérification précédant l’appel à
`UNUserNotificationCenter`. Le moteur extrait les trois parties du helper
signé dans un dossier temporaire privé, en lancement direct comme depuis
l’application installée. Ce fonctionnement existe aussi sur le tag v0.21.0.
Le chemin temporaire change entre processus.
La stabilité du consentement et la présentation d’une bannière sur une
session réelle restent à observer. La signature de qualification ad hoc
ne représente pas la future distribution Developer ID et notarisée.

La demande native du helper porte sur alerte et son. Son résultat et
l’acceptation d’une notification par le centre de notifications décrivent
deux événements distincts. Le son Glass est émis directement par AppKit
sur une autre commande. Il peut être audible avec les bannières désactivées.
Un état autorisé doit être complété par les réglages d’alerte et le style
d’affichage. Une suspension interne est vérifiable via la préférence de pause.
Une suspension par macOS doit être affichée uniquement lorsqu’elle est
observable. [Autorisation](https://developer.apple.com/documentation/usernotifications/asking-permission-to-use-notifications),
[réglages et style de l’alerte](https://developer.apple.com/documentation/usernotifications/unnotificationsettings/alertsetting).

Le contrôle du paquet compare les octets du helper embarqué avec les trois
éléments scellés, vérifie l’identité, la signature ad hoc, les entitlements
et les liens CoreAudio, CoreMediaIO, AVFoundation et UserNotifications.
Le contrôle des collecteurs exécute le CLI natif direct et le CLI issu du DMG.
Les deux déclarations d’usage décrivent l’observation passive. Le collecteur
CoreMediaIO peut signaler qu’un périphérique fonctionne sans identifier son
client ni établir le passage d’images. CoreAudio peut identifier un client
d’entrée si la source fournit une identité exploitable. Un runner sans
capteur peut produire un état dégradé ou indisponible accompagné du
diagnostic natif. Il ne qualifie pas Telegram ni le matériel du testeur.

Les paquets Intel et Apple Silicon de `18feb1a` ont passé l’audit du helper
et le cycle installé avec IPC 10, refus d’un sceau altéré, rollback et
retrait. Les deux ouvertures de Flutter par paquet ont reçu l’acquittement
du moteur natif. CoreAudio était sain et CoreMediaIO signalait un état
dégradé, en direct comme depuis le DMG, sur les deux runners. Le contrôle
du paquet n’a demandé aucune autorisation de notifications et n’a changé
aucun réglage TCC. [Job Intel](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44245427764/job/134603104653),
[job Apple Silicon](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44245427764/job/134603104252).

Empreintes recalculées sur les quatre archives téléchargées et comparées
au fichier `SHA256SUMS` publié par la construction.

```text
d7e6e99138654e8f8eb31236eb576477f35e1c74aa11161862bbad39d775b389  miccamwatch-hybrid-macos-x86_64.dmg
123c0b776f255584f47d7edc7ad5cf3626a66c9c61851760ccb0e42352d6bab0  miccamwatch-hybrid-macos-x86_64.tar.gz
907e483ff300794b731e1b6e432f879cc880425bbbbfe035ea05b5e0ea64531d  miccamwatch-hybrid-macos-aarch64.dmg
9e8290c96fe49906f8efd54516958eb192dc23c3539041a8c8a722c2f3bdd993  miccamwatch-hybrid-macos-aarch64.tar.gz
```

## Critères de réception pour l’étape 4

Source de R1 à R8 : remontée du test utilisateur sur un vrai Mac sous
MicCamWatch **v0.21.0**. Ces critères sont transmis à l’étape 4.

| Réf. | Retour et critère vérifiable |
| --- | --- |
| R1 | Son audible, aucune bannière observée, aucun souvenir de demande. Sur session macOS fraîche, afficher séparément état inconnu, non demandé, accordé et refusé. Avec autorisation, tester aussi alertes désactivées, style de bannière nul et suspension détectable. Comparer son interne, acceptation de la notification et bannière effectivement vue. |
| R2 | Micro Telegram attribué, caméra parfois observée sans client fiable, voyant système visible. Avec Telegram et une caméra réelle, vérifier que l’activité peut être affichée indépendamment de l’attribution. En absence de client renseigné par l’API, afficher « Application non identifiée ». Relever le voyant séparément lors de la campagne matérielle. |
| R3 | Fenêtre native v0.21.0 jugée trop grande. Sur Flutter v0.22.0, vérifier au premier lancement sur petit et grand écran que la fenêtre tient dans l’espace utilisable, que le minimum expose les commandes, que l’ouverture est stable et que la taille et la position restaurées restent à l’écran après changement de moniteur. |
| R4 | `Profile: Balanced` est peu explicite. Vérifier que chacun des quatre profils décrit son effet réel sur l’évaluation. Private applique Strict, Development applique Conservative, Balanced et Meeting appliquent Balanced. Si la politique explicite prévaut ou que deux profils produisent la même évaluation, l’explication doit le montrer. |
| R5 | Si les icônes sont retenues, tester les petites icônes vectorielles monochromes en clair/sombre, avec zoom et navigation clavier. Les libellés doivent rester lisibles et le comportement natif du menu doit rester vérifiable. |
| R6 | Afficher la raison, la portée et le réglage natif des permissions effectivement utilisées. Tester autorisation accordée, absente et refusée. Vérifier que les limitations annoncées correspondent aux collecteurs et à l’action demandée. |
| R7 | Créer des cas de test distincts pour aucune activité, observation incomplète, caméra inconnue, application non identifiée, protection demandée et protection confirmée. Le dernier état exige une lecture de confirmation du mécanisme natif. |
| R8 | Sous compte standard, exercer lecture, interface, préférences, notifications et maintenance par utilisateur sans élévation. Pour toute action exigeant une élévation, vérifier l’explication, la portée de l’action et l’état après refus ou annulation. |
