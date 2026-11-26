# Transfert Codex — MicCamWatch v0.22.0, fin de l’étape 4

Date : 26 novembre 2026. Lire avec
[le suivi de consolidation](HYBRID_STAGE4_CONSOLIDATION.md),
[l’audit](HYBRID_STAGE4_AUDIT.md) et
[le transfert Neon conservé](HYBRID_STAGE4_NEON_HANDOFF.md).

**ÉTAPE 4/6 : TERMINÉE.** La qualification MSI en compte standard et les
preuves logicielles des cinq cibles sont acquises. La dernière CI Windows
complète a réussi sur `935310d`. L’étape 5 attend une mission distincte.
Voir le [run 45784960802](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45784960802).

## Instructions du propriétaire qui font autorité

- Le propriétaire a accepté explicitement les limites matérielles ci-dessous
  **pour v0.22.0**. Elles ne deviennent pas des conditions physiques obligatoires
  de publication à l’étape 5. Leur absence seule ne doit pas bloquer indéfiniment
  la publication. Aucun nouveau matériel n’est présumé disponible ultérieurement.
- Les défauts critiques de sécurité ou de fonctionnement restent bloquants.
  Une limitation acceptée ne dispense pas de traiter un défaut critique établi.
- Ne déclarer aucune fonction corrigée sans preuve suffisante. **Vorssaint réel
  n’est pas déclaré corrigé.** Ne pas solliciter le testeur Mac.
- Terminer uniquement l’étape 4, puis fournir ce transfert. L’étape 5 n’est pas
  commencée. Pas de release, tag, merge vers main, winget ni déploiement du site.
- Modèle demandé en dernier : GPT-6.2 Sol, xhigh. Aucun autre fournisseur ni
  sous-agent n’a été lancé pendant cette reprise.

## Emplacements et Git

Le worktree `Documents/mcw-flutter-0220` du profil Windows porte la branche
`work/flutter-hybrid-0220`. Le dossier voisin `Documents/miccamwatch` reste sur
main local `73369ab` et contient les preuves dans `target`. Conserver ses
`.codex-attachments`.

Les 14 worktrees sont conservés, dont Neon sous
`miccamwatch/target/stage4-neon-review` et `target/website-dark`.
Stash conservé : `a373866fde847e96af75b38b24968fdb923c82d1`.
Main distant vérifié : `a0aee0fe04c6a8a0d07b7d5807879b9707711991`.
Tag stable v0.21.0 vérifié : `06d800ba536b5e6945abe4a4345957ba85c79cbf`.
Leur préservation n’autorise aucune future modification.

Les six commits Neon ont été intégrés en avance rapide de `16d3012` à
`3e07de0`, documentation incluse. `7bf4862` est le commit mécanique rustfmt.
Les correctifs suivants sont décrits dans le suivi, avec leurs preuves propres.
Pour le HEAD final de ce rapport, lire `git log -1` ; un commit documentaire ne
renomme jamais la révision embarquée dans un paquet déjà construit.

## Candidate de produit réellement qualifiée

- Source de construction : `2aae176454f0c38baae5e8b30029b14f9d46ef7c`.
- MSI : `miccamwatch-hybrid-windows-x86_64.msi`.
- SHA-256 : `b12719da3aa6673068ade756908a2e83846653afde55b6e43bf596f1e55d1582`.
- ProductCode : `{9343EA2E-03F3-4177-97ED-F65BC08EE44A}`.
- UpgradeCode : `{8FC9317B-4517-4E83-9F52-CF3257584BF6}`.
- Baseline officielle v0.21.0 : SHA-256
  `4e6145a4e48bff47f08c8341c24c14e7fab563146e36b8338f46441f7f4da3df`,
  ProductCode `{86CDFED5-BA07-4C91-8BC7-8A856198DC33}`.

Après `2aae176`, les changements portent sur les tests, leur pilotage, les
exemples de qualification et la documentation. Le seul ajout ultérieur dans
`src/platform/macos.rs` est sous `cfg(test)` ; le C ajouté est une fixture.
Le code distribué Rust/Swift/C++/Dart et Cargo.lock ne changent pas. Le nouvel
exemple Cargo sonore n’est pas distribué. Ne pas rebâtir ou réinstaller un MSI
pour une simple correction documentaire.
Les gardes automatiques de replay restent strictes : si un écart de Cargo.toml
ou de sources de tests empêche leur fast-path, ne pas les contourner ni
réétiqueter un ancien paquet. Conserver la provenance `2aae176` et reconstruire
la future candidate approuvée lorsqu’un nouveau paquet est réellement nécessaire.

## Preuves à conserver et statut des CI

| Domaine | Source, preuve et état à cette rédaction |
| --- | --- |
| Flutter Unix, quatre cibles | `2aae176`, [45774068870](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45774068870) : **4 jobs réussis** |
| Windows hybride | `2aae176`, [45774075203](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45774075203) : construction, Rust, paquet et scénario administrateur réussis ; compte standard hébergé refusé par DisableMsi=1 / 1625 ; run global échoué conservé comme tel |
| Windows standard natif | VM x64, reçu `lab-consolidated-20261126-074148` : **PASS**, trois transactions MSI code 0 |
| Native Linux x64/ARM64 | `6931781`, [45776813397](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45776813397) : **2 jobs réussis**, dont frontière root et transactions du helper caméra |
| Native Mac Intel/ARM | `cfa9880`, [45777558636](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45777558636) : **2 jobs réussis**, dont refus d’un autre utilisateur et fermeture AppKit réelle d’une fixture durcie |
| CI générale Windows antérieure | `5e01265`, [45778888517](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45778888517) : toutes les étapes réussies sauf le rendu des profils, refusé par le second garde historique de chemin Cargo |
| Profils et menus Windows requalifiés | `935310d`, [45783944115](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45783944115) : **réussi**, garde d’image partagé et 12 cas de refus/admission ; délais GUI conservés après compilation froide séparée |
| Dernière CI Windows complète | `935310d`, [45784960802](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/45784960802) : **réussi**, toutes les étapes du job Windows validées et six artefacts de preuve |
| Flutter local officiel | SDK 3.47.8 : analyse sans problème ; **131/131 tests** après `a0b40f0` |
| Rust maintenance Windows empaquetée | **34 tests réussis**, plus inspections exactes ZIP et MSI ; les six refus `os error 5` de Neon passent sur le runner isolé |

Le 1625 est une restriction native du runner et **pas une régression démontrée**.
Aucune politique Windows Installer n’est modifiée. La preuve standard manquante
sur ce runner est apportée par le vrai compte standard de la VM, sans déclarer
le run hébergé globalement réussi.

Les chemins des tests Cargo sont admis par le même helper dans le pilote des
profils et le renderer. Le chemin doit être sous target, correspondre au test
du dépôt (ancien format deps ou nouveau build/crate/hash/out), sans redirection.
Le renderer conserve les contrôles de PID, naissance, image, classe et fenêtre
visible. Le mode ciblé n’exempte aucun critère de la CI Windows normale.

Racine locale des preuves sous le profil Windows :
`Documents/miccamwatch/target/stage4-qualification`.
Sous-dossiers importants :

- `windows-2aae176` : MSI, ZIP, manifeste de bundle et SHA256SUMS.
- `consolidated-2aae176` : journaux, reçus et archives natives des quatre Unix
  et preuves Windows téléchargés. Les tar macOS préservent leurs modes Unix.
- `native-final/windows-full-935310d` : six artefacts du dernier run Windows,
  reçu `run.json`, journal du job `windows-139789307861.log` et index
  `sha256-evidence.json`. Le test des profils ciblé a son propre dossier
  `native-final/windows-profiles-935310d`.
- `lab-consolidated-20261126-074148/proof` : reçu brut, journaux des trois MSI,
  données du jeton standard, empreintes et stderr du propriétaire Rust/Flutter.
- `lab-consolidated-20261126-074148/independent-observations` : installation
  baseline, installation hybride, puis retrait hybride, observés par l’autre
  compte. État installé 5, versions et cache cohérents ; après retrait, 1605 et
  absence dans les trois contextes. Ne jamais conclure à partir des seuls fichiers.
- `lab-consolidated-20261126-074148/summary.json` : synthèse dérivée et index
  SHA-256 des preuves, sans remplacer les reçus bruts.
- `flutter-all-a0b40f0.log`, `host-stable-elevated-readonly.json`,
  `lab-final-state.json`, `checkpoint-consolidated.json` et
  `guest-preparation-diagnostic.json`.

Les pannes MSI de l’étape 3 n’ont pas été rejouées sans impact démontré :
la maintenance n’a changé que par formatage et mises à jour de dépendances.
Les contrats de refus, récupération, conservation des données et vérification
d’archives sont testés par les 34 cas empaquetés et les inspections exactes.

## Laboratoire après les tests

Le laboratoire `MCW-Win11-x64-Lab` exécute Windows AMD64 natif. Vérifier
son identité à partir des reçus locaux avant toute opération dans la VM.
La VM reste démarrée, isolée et au repos : aucun client MSI, application MCW
ou tâche MCW active au reçu final. Le produit testé a été désinstallé ; ses
données de test ont été conservées. Les données préexistantes QA ont été
préservées sous les dossiers `MCWStage4-*/preserved-before`, sans suppression.

Quatre checkpoints conservés, dont `01 - Windows 11 x64 propre` et le nouveau
`04 - Avant qualification consolidee 2aae176`, identifié dans
`checkpoint-consolidated.json`. Ne restaurer aucun checkpoint automatiquement.
Aucun VHDX/AVHDX n’a été supprimé ou déplacé.

Le contrôleur de cette reprise est terminé. PowerShell Direct a été retiré et
le credential uniquement en mémoire a été disposé. Une prochaine session peut
nécessiter UAC et la saisie du mot de passe du compte administrateur distinct
du laboratoire dans une PasswordBox locale ; jamais dans le chat, Git, la ligne
de commande ou un journal.

Le propriétaire a autorisé `RemoteSigned` **uniquement pour les processus de
test QA et le processus administrateur de lecture MSI dans cette VM**. Les
portées persistantes restent `Undefined` ; la politique effective finale est
`Restricted`. Les GPO et stratégies Windows Installer sont inchangées. Ne pas
étendre cette autorisation à l’hôte.
Le checkpoint ProductionOnly avait fermé la première session Direct ; la
reprise suivante a retrouvé la VM intacte sans réinstaller Windows.

Le script réutilisable est `installer/tests/windows-stage4-lab.ps1`, avec
`windows-stage4-observer.ps1`, `windows-hybrid-install.ps1` et
`windows-fixture-context.ps1`. Il exige une session authentifiée vers cette
VM, un paquet sous la racine de qualification et son empreinte indépendante.
Les scripts hôtes d’authentification de cette reprise restent dans target,
ignorés par Git. Ne jamais exécuter le scénario MSI sur le PC principal.

## Fonctions prouvées, limites acceptées et risques résiduels

| Fonction | Preuve logicielle et limite à conserver |
| --- | --- |
| Fermeture normale Mac | Action distincte avec confirmation, identité libproc/AppKit/session et sortie réellement observée. Fixture graphique durcie, refus puis acceptation. Un processus non graphique d’un autre utilisateur est refusé et reste vivant via son pipe privé ; cela ne constitue pas un essai d’application Aqua sous un autre compte. Une application peut attendre un document non enregistré ou refuser la demande. |
| Terminaison forcée / Vorssaint | task_for_pid reste soumis à macOS, SIP et Hardened Runtime. Le refus Mach 5 n’autorise aucun repli par PID brut ni sudo. Le cas réel Vorssaint n’est pas reproduit et n’est pas déclaré corrigé. Parcours : fermeture normale, menu propre de l’application, puis Force Quit macOS si disponible. |
| Identité et réutilisation PID | Identité de naissance périmée et cible protégée refusées ; instance AppKit retenue et comparée. Aucune revendication d’une réutilisation réelle de PID imposée au noyau. La liveness de la fixture autre UID vient de son pipe retenu, sans contourner le refus de lire ses métadonnées. |
| Caméra Mac | START/STOP connus, états partiels, erreurs et disparition/reconnexion testés en logiciel. Inventaire vide ou état inconnu = lacune, pas inactivité prouvée. Caméra active sans client attribuable : application inconnue, aucun PID fabriqué. Pas de flux physique ni de veille/réveil validé. |
| Notifications Mac | Helper signé ad hoc au chemin stable du bundle installé, trois blobs vérifiés identiques aux octets embarqués, autorisation consultable, file bornée et arrêt sonore testés. Soumission ≠ bannière. Persistance du consentement entre redémarrages/mises à jour, vrais refus/consentements et bannières réelles non qualifiés ; une identité ad hoc peut demander une nouvelle autorisation. Aucune modification TCC. |
| Géométrie | Sauvegarde/restauration du rectangle et borne à la zone de travail Windows/Linux. WM_DISPLAYCHANGE exercé dans la VM sur la fenêtre possédée. Aucun véritable retrait d’écran, changement matériel de DPI ni contrôle du placement Wayland revendiqué. |
| Accessibilité | Sept langues, deux thèmes, clavier, sémantique et zoom 200 % vérifiés. Pas de parcours complet VoiceOver/Narrator/NVDA/Orca revendiqué. |
| Performances | Mesures Neon AB/BA physiques conservées ; relevés supplémentaires VM, Linux Xvfb et Mac hébergé rattachés à leur contexte. Pas de GPU Linux matériel, microphone/caméra physiques actifs ou reprise matérielle après veille qualifiés. Aucune cadence de collecte réduite pour les chiffres. |

Ces absences de matériel sont **acceptées comme limites de qualification de
v0.22.0**, pas converties en preuves ni en conditions de publication obligatoires.
Les étapes 5/6 devront garder cette transparence et les messages de refus.
Un défaut critique effectivement démontré reste à corriger avant publication.

Les nouvelles versions correctives de cookie/url/zip/rustix/tokio sont
verrouillées. Les avertissements sans correctif de maintenance
chrono/zbus_names/zvariant/zvariant_derive restent visibles et constituent un
risque de maintenance à suivre ; aucun avis n’est ignoré pour rendre la CI verte.

## Mesures VM et protection du PC principal

Une seule séquence VM, 16 points par état pendant environ 15 s, sans capture
active : elle ne remplace pas les benchmarks AB/BA Neon.

| État observé | Processus | CPU % d’un cœur | RSS médiane Mio | Privé médian Mio |
| --- | ---: | ---: | ---: | ---: |
| v0.21.0 tray seul | 1 | 5,449 | 17,529 | 2,932 |
| v0.21.0 fenêtre native ouverte | 1 | 5,972 | 26,117 | 3,664 |
| v0.22.0 Rust + Flutter ouvert | 2 | 9,007 | 91,604 | 64,244 |
| v0.22.0 après fermeture Flutter | 1 | 1,653 | 16,320 | 3,293 |

Sortie de Flutter observée en 237 ms. Le journal Rust rapporte une disponibilité
IPC à 3 550 ms après ouverture à froid dans cette VM ; le champ brut 4 462 ms
incluait aussi la sonde de géométrie. Le champ baseline 443 ms ouvre un tray
déjà prêt : **ce n’est pas une comparaison d’ouverture équivalente**. Le
harnais a été corrigé pour isoler la mesure et décrire cette différence.
Ne pas déduire de gain CPU général de cette courte séquence sérialisée.

L’hôte garde la v0.21.0 enregistrée au ProductCode stable, les deux empreintes
historiques identiques et le tray PID 18072, lancé avant cette mission. Aucun
MSI, CLI de contrôle, autostart, préférence, historique ou protection de l’hôte
n’a été modifié ou arrêté par la mission. Les vérifications sont en lecture seule.

Disque C: 39,07 Gio libres avant travaux ; seul cache autorisé supprimé :
`miccamwatch/target/debug/incremental`, 15,051 Gio logiques, 13,30 Gio réellement
libérés. Immédiatement après : 52,38 Gio libres. Les preuves téléchargées et
compilations ultérieures consomment une partie du gain. Pas de nettoyage global.

## Reprise dans une nouvelle session

Commencer par lire ce rapport et le statut final de consolidation, puis vérifier
branche, HEAD, propreté, résultats CI référencés et espace disque. Ne pas refaire
l’audit des 20 domaines, les benchmarks Neon, la campagne MSI de l’étape 3 ni
la qualification MSI standard acquise sans nouvelle modification pertinente.

Attendre une mission explicite de l’étape 5. Ne publier aucune v0.22.0, ne
changer ni main, ni la stable, ni les autres VM/worktrees/stash. Toute future
qualification doit rester rattachée à ses vrais octets, son compte et sa révision.
