# MicCamWatch v0.22.0 — diagnostic du rollback MSI par utilisateur

Le [contrat de maintenance MSI](WINDOWS_MSI_MAINTENANCE.md) contient
les mesures Hyper-V x64 sur les MSI produit. Après retrait, le relevé
administrateur renvoie `1605`. Dans le compte d’installation, le même
ProductCode peut rester annoncé (`State=1`) sans cache ni index
UpgradeCode. Ce cas exige la vérification du PackageCode et une
réinstallation du MSI authentifié avant tout retrait ciblé.

## Conclusion

Le défaut est reproductible dans Windows Installer avec un paquet qui
installe un fichier texte. Il concerne la restauration de l’enregistrement
d’un produit désinstallé dans un major upgrade par utilisateur sur
Windows 11 ARM64 hébergé. Le journal montre des copies de sauvegarde
et des écritures de registre refusées pendant la même phase de rollback.
L’observation indépendante retrouve le fichier
mais ne retrouve plus le produit antérieur. La cause la plus probable
est l’application des droits du compte impersonné à certaines opérations
de reprise qui doivent restaurer des entrées et propriétaires protégés.
La preuve du jeton effectivement utilisé par le noyau sur chaque appel
reste à établir.

**ÉTAPE 3/6 : INCOMPLÈTE.** Aucun changement de production n’est retenu.
Les effets du défaut sur le compte standard restent critiques et le choix
d’un nouveau contrat d’installation nécessite un essai réellement atomique.

## Chronologie du produit exact

Les deux relectures utilisent le MSI SHA-256
`2d5b508c08baf660bbcf8a27a7ab601b4ec6c5185314666502f9bca16540a110`
de `7e5c1fff1e90a7714f4e2c37d3900fdfa5a3864a`. Le banc de mesure est à
`071c2d1cd9e41b2aae78b95a86f90c5c62af2928`. Le scénario historique a
été observé une première fois sur le banc `fee9514`.
[Construction et preuves MSI/ZIP x64](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44131699871/job/134210242949),
[rollbacks historiques](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44245924802).

| Étape | Observation |
| --- | --- |
| État initial | `MsiGetProductInfoExW` annonce l’état installé `5` dans `USERUNMANAGED` pour le SID du compte standard. Le cache MSI, le chemin `%LOCALAPPDATA%\Programs\MicCamWatch` et l’index d’upgrade utilisateur concordent. Les ProductCode sont `{86CDFED5-BA07-4C91-8BC7-8A856198DC33}` (v0.21.0) et `{AF3F394D-F615-4D97-815F-CF8873964A6B}` (v0.22.0). Leur UpgradeCode est `{8FC9317B-4517-4E83-9F52-CF3257584BF6}`. |
| Admission | Le client lance `msiexec.exe /i … /qn /norestart`. Les journaux client `MSI(c)` transfèrent l’opération au serveur `MSI(s)`. `FindRelatedProducts` identifie l’ancien produit pour le même utilisateur. |
| Retrait | Le script d’exécution du MSI hybride place `InstallInitialize` à 1500 et `RemoveExistingProducts` à 1501. Le retrait ouvre le MSI antérieur depuis le cache, entre en maintenance et exécute sa propre séquence de désinstallation. Le retrait se termine avec valeur 1. |
| Panne | Dans les copies de qualification, l’action immédiate de type 19 à 1502 rend la valeur 3. Les deux MSI d’origine ont une action de type 51, `SetARPINSTALLLOCATION`, qui fixe un chemin. Ils ne contiennent ni l’action de panne ni de code personnalisé de restauration. |
| Rollback | Le journal serveur ouvre un script `ScriptType=2`. Les lignes `FileCopy` réutilisent les sauvegardes `.rbf`. Les empreintes du CLI, du tray et, pour le bundle hybride, du manifeste et de la charge Flutter correspondent à l’état antérieur. Les préférences, l’historique et l’autostart sont préservés. |
| Registre | Refus 1401/1406 sur `Installer\UserData\<SID>\Products\<produit>\{Patches,Features,InstallProperties,Usage}`, `Installer\TempPackages`, l’index machine `UpgradeCodes` et les clés de désinstallation. Le code système 5 désigne le refus d’accès. Sous `HKEY_USERS\<SID>\Software\Microsoft\Installer\UpgradeCodes`, le code 1307 signale un propriétaire invalide lors de la recréation de la clé. |
| État final | Le client retourne `1603`. Pour l’ancien ProductCode, `MsiGetProductInfoExW` retourne `1605`, l’entrée dans l’index utilisateur a disparu et `MsiEnumProductsExW` retourne `259` dans les trois interrogations ciblées (utilisateur géré, utilisateur non géré, machine). La restauration de l’enregistrement a échoué. |

La lecture du cache et des tables MSI a été limitée à des ouvertures en
lecture seule. Le contexte machine a été interrogé avec un SID nul.
L’API d’énumération ne permet pas de conclure à l’absence d’une annonce
par utilisateur non gérée appartenant à un autre SID. Le résultat
`1605` et l’index confirment ici la perte de l’installation précédemment
enregistrée pour le SID qui l’a effectuée.
[Actions et position du retrait](https://learn.microsoft.com/en-us/windows/win32/msi/removeexistingproducts-action),
[lecture de l’état](https://learn.microsoft.com/en-us/windows/win32/api/msi/nf-msi-msigetproductinfoexw),
[énumération par contexte](https://learn.microsoft.com/en-us/windows/win32/api/msi/nf-msi-msienumproductsexw).

## Contexte de sécurité observé

- Les deux essais du produit exact utilisent Windows `26200.9974` et
  `msi.dll` `5.0.26100.9974`. Le PowerShell client appartient au compte
  standard jetable, avec profil chargé, jeton primaire, intégrité moyenne
  `S-1-16-8192` et absence du groupe Administrateurs. L’Explorateur
  visible appartient au compte hébergé, pas au compte de test.
- `IsWow64Process2` indique `0000/AA64` pour les processus clients
  et le service. Le moteur MSI exécute du code ARM64 natif même pendant
  l’installation d’un paquet dont le template est `x64`.
- `msiserver` s’exécute en `LocalSystem`, SID `S-1-5-18`, intégrité
  système. Le worker correspondant aux refus du rollback hybride,
  PID 1812 et TID 3872, est échantillonné avec un jeton
  d’impersonation de type 2, niveau `SecurityImpersonation` (2), pour
  le SID standard. Ce jeton a `SeChangeNotifyPrivilege` et
  `SeShutdownPrivilege` activés. Les privilèges échantillonnés sur le
  jeton primaire système, dont `SeRestorePrivilege` parfois actif,
  n’apparaissent pas sur ce jeton de thread.
- Des échantillons du même thread retournent `ERROR_NO_TOKEN` (1008).
  La trace faible fréquence donne des intervalles premier/dernier
  échantillon, pas une preuve par appel de registre. Les étiquettes
  PID/TID du journal MSI sont tronquées aux octets bas et peuvent
  coïncider. Dans l’essai v0.21.0, TID 11008 porte l’impersonation dans la
  fenêtre utile et TID 1792 possède le même octet bas. Le cas hybride
  permet une corrélation plus nette.

Le collecteur lit les métadonnées de processus et les jetons de
`msiexec.exe` avec `TOKEN_QUERY`, dans l’environnement de test. Il
n’active aucun privilège et ne change ni jetons ni ACL. Les relevés
contiennent des SID et chemins issus de comptes jetables.
[Rejeu depuis v0.21.0](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44283533714/job/134730190274),
[rejeu hybride](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44283533714/job/134730190811),
[journaux et jetons v0.21.0](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44283533714/artifacts/14417887339),
[journaux et jetons v0.22.0](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44283533714/artifacts/14417174031),
[OpenThreadToken](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-openthreadtoken).

## Reproduction minimale et hypothèses

Le workflow `44282461222`, banc `4b3fd178733c9f2b0c6727f05fabc952dd5e5366`,
a installé quatre produits jetables sous compte standard. Chaque MSI est
généré par le WiX épinglé du dépôt et contient un petit fichier texte et
un marqueur HKCU. Il n’utilise ni Rust, ni Flutter, ni le ProductCode ou
les chemins de MicCamWatch. Les quatre pannes ont suivi le retrait
effectif et déclenché un vrai rollback. Les critères d’enregistrement
maintiennent le job en échec.

| Variante | Panne et résultat vérifié |
| --- | --- |
| x64, retrait après `InstallInitialize` | Type 19. Fichier ancien restauré, ancien et nouveau ProductCode absents, 78 refus 1401 avec codes système 5 ou 1307. |
| x64, retrait après `InstallInitialize` | Cabinet externe volontairement absent lors de l’exécution d’`InstallFiles`. Erreur native 1311 durant `InstallFinalize`. Même résultat, 78 refus. |
| ARM64, retrait après `InstallInitialize` | Même erreur 1311 avec template `Arm64` et moteur natif. Même résultat, 78 refus. |
| x64, retrait après `InstallExecute` | Type 19 après succès du retrait à 6501. Fichier ancien restauré, produit ancien absent et **nouveau produit v2.0.0 encore enregistré** au même chemin (`State=5`, index présent). 137 refus. |

L’absence de code MicCamWatch dans la reproduction exclut une dépendance
au transfert Rust, à Flutter, aux scripts de sauvegarde applicatifs et à
l’action injectée de type 19. Le cas ARM64 avec erreur 1311 exclut une
dépendance à l’émulation x64. Le retrait tardif ne corrige pas la perte et
peut laisser un enregistrement correspondant à une autre version que
le fichier sur disque. Les essais ne séparent pas encore un comportement
de cette build Windows d’un effet propre au runner ou au type de session.
[Job des quatre variantes](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44282461222/job/134726439884),
[journaux, sources WiX et reçus](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44282461222/artifacts/14418055891),
[erreur MSI 1311](https://learn.microsoft.com/en-us/windows/win32/msi/windows-installer-error-messages).

## Correction et qualification restante

Les fichiers changés portent sur les deux workflows et les scripts
`installer/tests/windows-{hybrid-install,hybrid-standard,hybrid-registration,msi-probe-build,msi-probe-install}.ps1`
ainsi que `installer/tests/windows-msi-token-samples.cs`. Le contrat du
paquet et le code exécuté par le produit restent identiques.
`installer/hybrid_windows.py` fixe la séquence existante et
`src/updater/msi.rs` valide la version, la portée, le chemin et
l’enregistrement après retour de Windows Installer. `src/updater/hybrid.rs`
conserve l’état `intervention_required` quand cette vérification échoue.

Le retrait après `InstallValidate` détruirait l’enregistrement avant
l’ouverture de la transaction. Le retrait après `InstallFinalize` sortirait
le nettoyage de l’ancienne version de la transaction du nouveau produit.
Le retrait après `InstallExecute` a été essayé sur la fixture avec GUID de
composant, architecture et chemin identiques. Il a laissé un enregistrement
incohérent. Aucun de ces trois déplacements ne constitue une correction
qualifiée. Une transaction multipaquet native constitue une expérience
distincte à évaluer seulement après comparaison hors runner.
[Règles du major upgrade](https://docs.firegiant.com/wix/schema/wxs/majorupgrade/),
[transactions multipaquets MSI](https://learn.microsoft.com/en-us/windows/win32/msi/multiple-package-installations).

Le cycle positif standard ARM64, le MSI administrateur x64, le ZIP,
la reprise après interruption, l’IPC 10 et l’arrêt coopératif de
Flutter conservent leurs preuves précédentes. Les chemins produit et Unix
sont identiques à leurs révisions déjà qualifiées. Les quatre scénarios
minimaux échouant au contrôle de la restauration n’exercent pas
la nouvelle tentative ni la désinstallation après rollback. Les futurs
essais positifs doivent exercer ces deux opérations.
[Témoin standard](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44246770527/job/134607504582),
[Windows administrateur et ZIP](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44131699871/job/134210242949),
[bilan multiplateforme](HYBRID_DISTRIBUTION.md).

## Rejouer sur une VM Windows x64 jetable

Le dépôt ne dispose d’aucun runner x64 dédié. Les runners Server 2022 et
Server 2025 déjà essayés appliquent `DisableMsi=1` et renvoient 1625.
L’accès de lecture à la VM Hyper-V locale est refusé par le poste hôte.
Windows Sandbox est activé. Une nouvelle session dédiée est refusée
parce qu’une session est déjà ouverte (`CO_E_APPSINGLEUSE`). Le marqueur
du laboratoire est absent de la session active. Aucun MSI n’y a été
exécuté et la session a été laissée ouverte.
Les validations d’annulation ou de refus UAC dans une vraie session
standard restent à faire. [Procédure VM](WINDOWS_STANDARD_USER.md#cas-interactifs-et-contrôle-de-lenregistrement).

1. Construire uniquement les fixtures avec `dotnet tool restore` et
   `installer/tests/windows-msi-probe-build.ps1 -Output <dossier neuf>`.
   Conserver `probes.json`, les `.wxs` et les SHA-256. La compilation
   locale des douze fixtures et la lecture de leurs séquences ont réussi.
2. Sur un Windows 11 x64 jetable dont la politique autorise déjà les
   installations utilisateur, transférer `probes.json` et les trois MSI
   de la variante `x64-early-media` dans un dossier local réservé à
   l’essai. Ne transférer aucun `probe-media.cab` pour le MSI `fault`.
3. Dans le bureau du compte standard, installer le MSI `baseline`
   avec `msiexec.exe /i "<baseline.msi>" /qn /norestart /l*v "<baseline.log>"`.
   Vérifier le SID, l’empreinte du fichier texte et l’état `5`.
   Depuis un compte observateur distinct, utiliser
   `Get-McwMsiRegistration` avec les trois paramètres du plan
   (ProductCode, SID, UpgradeCode) puis `Get-McwMsiContexts`.
4. Installer le MSI `fault` avec un journal neuf. Rechercher
   `RemoveExistingProducts`, 1311, `ScriptType=2` et les erreurs 1401.
   Relever le même enregistrement et fichier après 1603. Conserver
   les reçus avant toute désinstallation, réparation ou restauration de VM.
   Si l’état ancien a été pleinement restauré, rejouer le MSI `good`
   et vérifier son retrait. Pour la qualification produit, ajouter
   les scénarios interactifs et ceux du MSI exact du guide Windows.

La prochaine comparaison discriminante est le résultat de cette même
fixture sur un Windows 11 x64 natif, en session standard interactive avec
politique inchangée. La question technique à soumettre ensuite est le
mécanisme pris en charge par Windows Installer pour restaurer, sous ce
jeton d’impersonation, les clés et propriétaires d’un produit retiré
avec `RemoveExistingProducts` dans la même transaction, ou la build
Windows qui corrige cette restauration.

Le registre de désinstallation du poste principal affiche encore
v0.21.0. Les empreintes du CLI et du tray correspondent aux deux
exécutables v0.21.0 mesurés sur le runner et le tray tourne depuis ce
chemin. Les douze worktrees, `main`, le tag public, la candidate
historique et le stash ont été vérifiés en lecture seule.

## 19 novembre 2026 — transaction multipaquet et récupération

### Disponibilité du x64 natif

L’inventaire actuel GitHub ne contient aucun runner dédié. Les deux runners
Windows Server x64 de référence avaient refusé les MSI non gérés par
`DisableMsi=1` (`1625`). La lecture de la VM Hyper‑V désignée est toujours
refusée. L’inventaire Windows Sandbox était vide. Aucun invité x64
jetable avec un bureau interactif appartenant à un compte standard n’a
été admis. Le bac à sable et Hyper‑V n’ont fait l’objet d’aucune
modification de configuration. Le jeu de trois MSI x64 déjà préparé
reste disponible pour le rejouer selon la procédure ci-dessus.

### Test discriminant sur ARM64

Le banc `444209f4825629f86b4fcd3c5cbc2f72f4bef4e7` a repris la seule variante
`x64-early-media` des fixtures existantes. Le runner séparé a construit les
trois MSI de ce cas à partir de la même source WiX. Les octets du jeu local
et les trois autres variantes sont restés hors de cet essai. Le build était
`26200.9974` et `msi.dll` `5.0.26100.9974`. Le compte jetable disposait d’un
jeton primaire standard non élevé, à intégrité moyenne `S-1-16-8192`.
Les stratégies `DisableMsi` et `DisableRollback` recherchées étaient
absentes. Le journal confirme aussi `AlwaysInstallElevated=0` pour les deux
portées. Le service et le moteur échantillonnés étaient ARM64 natifs, même
avec le template MSI `x64`. [Job](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44598008530/job/135821445915) ·
[Artefact](https://github.com/Roman-Cuisset/miccamwatch/actions/runs/44598008530/artifacts/14565973744).

| Phase | Résultat |
| --- | --- |
| État initial | Le compte standard installe v1.0.0. L’observateur vérifie l’état installé, le cache, le chemin, le SID et l’index utilisateur. |
| Ouverture et retrait | `MsiBeginTransactionW=0` et `MsiConfigureProductExW(..., INSTALLSTATE_ABSENT)=0`. Le marqueur disparaît. Le journal confirme que le retrait et l’installation suivante appartiennent à la même transaction. |
| Panne | `MsiInstallProductW=1603` après l’erreur native 1311 sur le cabinet externe absent. Le retrait préparatoire est une opération de premier niveau. La transaction ne dépend pas de la désinstallation imbriquée du major upgrade du candidat. |
| Clôture | `MsiEndTransaction(ROLLBACK)=0`. Le marqueur retrouve son SHA-256 initial depuis le fichier de sauvegarde `.rbf`. Le relevé compte 78 erreurs 1401 avec un code système 5 ou 1307. |
| État MSI | Les deux ProductCode renvoient 1605. Les trois interrogations `MsiEnumProductsExW` donnent 259 et aucune des deux entrées ne figure dans l’index utilisateur. Le registre est perdu malgré le code zéro retourné à la clôture. |
| Récupération séparée | Le banc retente le MSI v1.0.0 d’origine, contrôlé par SHA-256, sous le même compte. `RegisterProduct` atteint la vérification `SECUREREPAIR` puis renvoie 1602. Le journal signale que l’invite d’élévation est indisponible en mode silencieux. Les deux enregistrements restent absents. La mise à niveau et le retrait après récupération n’ont pas été qualifiés. |

Les reçus et la trace en lecture seule sont archivés ensemble. Le journal
de réinstallation ne constitue pas une observation d’une boîte de dialogue
UAC ni d’un refus par une personne. Le code `1602` est le résultat enregistré
du parcours silencieux.

| Fichier | SHA-256 |
| --- | --- |
| `baseline.log` | `106edb7abd228a4bf535203d542ed8fdea565ed32316c654d2902a9caffa7461` |
| `failure.log` | `a94e7080e6ccb79a9d3270ff629443dde7f94a9fa27c64e6fb20c45cd3445688` |
| `reinstall-baseline.log` | `e38fd47c2a1ea74f1d0ffeb60ed9d6d20d179e01caa75ed6c4e9386ed48819aa` |

### Décision pour MicCamWatch

- L’installation `perUser` maintient l’identité du profil. Un changement
  vers `perMachine` modifie le contexte de sécurité et d’enregistrement de
  la migration. [Contextes MSI](https://learn.microsoft.com/en-us/windows/win32/msi/installation-context).
- Une mise à jour mineure conserve le ProductCode et ne permet pas cette
  migration du template MSI historique `Intel;0` au template `x64;0`.
  Microsoft exige des ProductCode distincts pour les packages 32 et
  64 bits. [Mise à jour mineure](https://learn.microsoft.com/en-us/windows/win32/msi/minor-upgrades) ·
  [Règles ProductCode](https://learn.microsoft.com/en-us/windows/win32/msi/product-codes).
- La transaction multipaquet utilise des API prises en charge et préserve
  l’identité du compte. Le défaut apparaît encore avec le retrait séparé.
  La désinstallation imbriquée du major upgrade n’est donc pas une
  condition nécessaire à la perte d’enregistrement sur cette build.
  [Transactions multipaquets MSI](https://learn.microsoft.com/en-us/windows/win32/msi/multiple-package-installations) ·
  [Clôture de la transaction](https://learn.microsoft.com/en-us/windows/win32/api/msi/nf-msi-msiendtransaction).
- La réinstallation silencieuse ne constitue pas une récupération
  qualifiée. Le maintien de `intervention_required`, des journaux et des
  contrôles indépendants reste le contrat de maintenance. Le MSI et le ZIP
  de production, le code Rust partagé et les packages Unix sont identiques
  à leurs révisions déjà évaluées.

**ÉTAPE 3/6 : INCOMPLÈTE.** L’expérience suivante reste la fixture
`x64-early-media` sur un Windows 11 x64 natif et jetable avec une session
standard interactive. Sur un second point de contrôle de cette VM,
exercer la réinstallation du MSI v1.0.0 exact via l’interface ordinaire
de Windows Installer après une perte d’enregistrement vérifiée.
Consigner le consentement éventuellement proposé par Windows et valider
le SID, le cache, le fichier, les deux ProductCode et l’index avant de
tenter un nouvel upgrade puis un retrait.

Pour une revue ciblée, établir quelle version de Windows ou quel contrat
MSI documenté permet de retrouver l’enregistrement `USERUNMANAGED`
lorsque le rollback natif de la transaction renvoie déjà zéro. Si seule
une reprise via `SECUREREPAIR` est possible, vérifier dans quelles
conditions l’identité du compte et l’intégrité du bundle sont préservées.
