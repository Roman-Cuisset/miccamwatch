# Préparation de la diffusion de MicCamWatch v0.22.0

Ce document prépare les opérations de l’étape 6. Le propriétaire doit
approuver la publication après lecture du rapport de qualification.
[Rapport de la candidate](HYBRID_STAGE5_RELEASE_CANDIDATE.md).

## GitHub Releases

1. Retrouver le run de qualification et le SHA exact approuvé. Vérifier la
   matrice, les résultats de la VM, les huit paquets, le SBOM SPDX,
   `SHA256SUMS` et les neuf attestations. Vérifier chaque sujet contre le
   dépôt, le chemin `.github/workflows/ci.yml`, le ref de branche, le SHA et
   le runner GitHub hébergé. Conserver l’archive et les preuves localement.
2. Vérifier les références distantes et la version publique. Le workflow
   `release.yml` en place réagit aux tags `v*` et assemble les anciens
   paquets natifs. Avant création du tag, préparer et faire valider sa
   neutralisation pour v0.22.0 ou une promotion contrôlée des octets déjà
   qualifiés. Un tag sur la révision candidate ne bénéficie pas d’un correctif
   apporté uniquement sur un commit ultérieur. Vérifier les déclencheurs
   effectifs avant toute mutation de référence.
3. Après accord, ancrer le tag au SHA de construction approuvé. Si une
   intégration à main est demandée, préserver la filiation de la candidate
   et traiter les conflits à part. Un commit de merge distinct n’est pas le
   SHA inscrit dans les paquets. Préparer un brouillon GitHub avec les dix
   fichiers exacts et les notes de version. Éviter toute reconstruction
   implicite et distinguer l’attestation de branche d’une attestation de tag.
4. Relire l’ensemble du brouillon, les noms, les tailles, les empreintes,
   les signatures et les limites. Les liens relatifs des notes deviennent
   des liens vers les documents au SHA approuvé. Une fois autorisé, publier
   et contrôler les dix fichiers téléchargés depuis GitHub, le tag, et
   l’état stable/latest. L’ancienne version reste téléchargeable.
   [Notes de version](RELEASE_0.22.0_NOTES.md).

## Site statique

Le site v0.21.0 propose un script Unix qui résout `latest` et attend des
archives natives. Avant que v0.22.0 devienne latest, valider une transition
qui laisse la commande v0.21.0 explicitement épinglée pendant la bascule.
L’ancienne commande échouerait sur les noms et le contenu hybrides.
Les anciens clients Windows ne doivent pas découvrir un ZIP hybride sous
le nom historique du ZIP natif.

Adapter les liens, exemples SHA et commandes des sept langues à la nouvelle
liste d’artefacts. Les liens de vérification doivent montrer les noms exacts
ci-dessous et la source réellement qualifiée. Le format actuel de
`site/release.json` décrit le script historique. Faire évoluer son contrat
avec le nouveau parcours d’installation, vérifier la CSP, la page de
confidentialité, le thème, le rendu mobile, canonical, hreflang et sitemap.
Exécuter les tests Node et le build statique dans une prévisualisation.
Après publication, contrôler les téléchargements et les empreintes sur le
site public. Les changements de références et le déploiement restent soumis
à l’accord de l’étape 6.

### Texte préparé pour les sept langues

| Langue | Titre et annonce | Installation et limites |
| --- | --- | --- |
| Français | **MicCamWatch v0.22.0 — une nouvelle fenêtre de consultation.** La fenêtre Flutter offre sept langues et deux thèmes. Le moteur Rust poursuit la surveillance et conserve l’icône de zone de notification après sa fermeture. | Choisissez le paquet hybride correspondant à votre système. Sous Windows, utilisez le MSI pour mettre à niveau une installation MSI v0.21.0. L’observation et les contrôles dépendent du système et des autorisations. Consultez les limites et les sommes SHA-256. |
| English | **MicCamWatch v0.22.0 — a new consultation window.** The Flutter window has seven languages and two themes. The Rust monitor and native tray remain running when you close it. | Choose the hybrid package for your system. On Windows, use the MSI to upgrade an existing v0.21.0 MSI installation. Observation and controls depend on the platform and permissions. Read the limitations and verify SHA-256. |
| Deutsch | **MicCamWatch v0.22.0 – ein neues Übersichtsfenster.** Die Flutter-Oberfläche bietet sieben Sprachen und zwei Designs. Die Rust-Überwachung und das Symbol im Infobereich bleiben beim Schließen aktiv. | Wählen Sie das Hybridpaket für Ihr System. Aktualisieren Sie unter Windows eine vorhandene MSI-Installation von v0.21.0 mit dem MSI-Paket. Erkennung und Steuerung hängen vom System und seinen Berechtigungen ab. Beachten Sie die Einschränkungen und prüfen Sie die SHA-256-Prüfsummen. |
| Español | **MicCamWatch v0.22.0: nueva ventana de consulta.** La interfaz Flutter incluye siete idiomas y dos temas. Al cerrarla, el monitor Rust y el icono del área de notificación siguen activos. | Elige el paquete híbrido de tu sistema. En Windows, usa el MSI para actualizar una instalación MSI de v0.21.0. La detección y los controles dependen del sistema y sus permisos. Consulta las limitaciones y comprueba las sumas SHA-256. |
| Русский | **MicCamWatch v0.22.0 — новое окно просмотра.** Интерфейс Flutter доступен на семи языках и поддерживает две темы. После закрытия окна монитор на Rust и значок в области уведомлений продолжают работать. | Выберите гибридный пакет для своей системы. В Windows обновляйте установку v0.21.0, выполненную через MSI, с помощью нового MSI. Обнаружение и управление зависят от системы и её разрешений. Ознакомьтесь с ограничениями и проверьте суммы SHA-256. |
| 日本語 | **MicCamWatch v0.22.0 — 新しい確認ウィンドウ。** Flutter の画面は 7 言語と 2 種類のテーマに対応します。ウィンドウを閉じても、Rust による監視と通知領域のアイコンは動作し続けます。 | お使いのシステムに合ったハイブリッドパッケージを選んでください。Windows で MSI 版 v0.21.0 を更新する場合は、MSI を使用してください。検出と制御は OS と権限に依存します。制限事項を確認し、SHA-256 チェックサムを照合してください。 |
| 简体中文 | **MicCamWatch v0.22.0 — 全新的查看窗口。** Flutter 界面支持七种语言和两种主题。关闭窗口后，Rust 监控和系统托盘图标仍会继续运行。 | 请选择与系统匹配的混合安装包。Windows 上通过 MSI 安装的 v0.21.0，请使用新版 MSI 升级。检测和控制取决于操作系统及权限。请阅读已知限制并校验 SHA-256。 |

## WinGet

| Champ | Cible de préparation |
| --- | --- |
| Identifiant | `RomanCuisset.MicCamWatch` |
| Version | `0.22.0` |
| Type et périmètre | WiX, utilisateur, x64, sous réserve d’inspection du MSI public |
| Nom du fichier | `miccamwatch-hybrid-windows-x86_64.msi` |
| Adresse et empreinte | Télécharger depuis la release publique approuvée et vérifier l’entrée nommée de `SHA256SUMS` ainsi que l’attestation |
| Identifiants MSI | Lire ProductCode et UpgradeCode sur le MSI public et contrôler l’inscription en VM |
| Dépendances | Décider selon l’inspection des imports de tout le MSI, Rust et Flutter compris, et le test en environnement propre |

Le générateur accepte le nom hybride explicitement. Utiliser le SHA-256 du
MSI public après vérification, dans un dossier de travail distinct.

```powershell
./distribution/winget/Generate-Manifests.ps1 `
  -Version 0.22.0 -InstallerSha256 $VerifiedPublicMsiSha256 `
  -InstallerFileName miccamwatch-hybrid-windows-x86_64.msi -OutputRoot $DraftDirectory
```

Vérifier le schéma WinGet en vigueur au moment de la soumission et tester
installation, mise à niveau et retrait depuis une session standard dans
la VM. Envoyer la mise à jour dans une mission autorisée et attendre sa
présence réelle dans l’index.
[Documentation WinGet](WINGET.md).
