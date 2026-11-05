# SECTOOL

Voir **tout son réseau en temps réel** et le contrôler : appareils (box, ordinateurs, téléphones, imprimantes, objets connectés), connexions de la machine, trafic, et un **petit pare-feu logiciel** avec aperçu avant application.

Application de bureau pour **Windows, macOS et Linux** : Rust (moteur) + Tauri 2 (fenêtre) + Astro / Vite (interface).

## Ce que fait SECTOOL

| Section | Contenu |
|---|---|
| Carte du réseau | La box au centre, chaque appareil avec son icône, son statut, et un bouton « Bloquer » |
| Appareils | Tableau : IP, MAC, constructeur, ports ouverts, latence, recherche, export JSON |
| Connexions | Sockets TCP/UDP de la machine avec le programme, la zone (Internet / local) et l'état |
| Pare-feu | Règles bloquer/autoriser, aperçu exact des commandes, application, retrait complet |
| Journal | Nouvel appareil, appareil hors ligne, changement d'adresse MAC (usurpation possible) |

La découverte n'utilise **aucun pilote ni capture de paquets** : table ARP, sondes TCP/UDP classiques et mDNS. Elle fonctionne sans droits administrateur.

## Pare-feu : sécurité

- Les règles sont enregistrées dans `firewall.json` et **ne touchent le système qu'après « Appliquer »**.
- Un aperçu montre les commandes exactes avant exécution.
- Windows : `netsh advfirewall`, règles nommées `SECTOOL|…`. Linux : table `nftables` dédiée `inet sectool`. macOS : ancre `pf` dédiée `sectool`.
- SECTOOL ne modifie **jamais** les règles qui ne lui appartiennent pas. « Tout retirer » supprime uniquement les siennes.
- L'application demande l'autorisation administrateur (UAC, `pkexec`/`sudo`, mot de passe macOS) uniquement au moment d'appliquer.
- Surveillez votre réseau uniquement : n'utilisez SECTOOL que sur un réseau qui vous appartient ou que vous administrez.

## Lancer en développement

Prérequis : Rust stable, Node 20+, et sous Linux `libwebkit2gtk-4.1-dev`.

```bash
npm --prefix app install
npm --prefix app run dev        # interface seule dans le navigateur (données de démonstration)
cargo install tauri-cli --version "^2"
cargo tauri dev                 # application complète
```

Ligne de commande (sans interface) :

```bash
cargo run -p sectool-cli -- scan
cargo run -p sectool-cli -- watch --interval 3
cargo run -p sectool-cli -- connections
cargo run -p sectool-cli -- preview
```

## Construire les installeurs

```bash
cargo tauri build
```

Le workflow `.github/workflows/release.yml` produit `.msi`/`.exe`, `.dmg` et `.deb`/`.AppImage` à chaque tag `vX.Y.Z`.

## Structure

```
crates/sectool-core      types, règles, base de constructeurs (OUI)
crates/sectool-net       ARP, sondes, mDNS, sockets, trafic
crates/sectool-firewall  netsh / nftables / pf
crates/sectool-cli       commande `sectool`
src-tauri                application de bureau (Tauri)
app                      interface Astro + Vite
```

## Licence

AGPL-3.0-or-later. Icônes : [Lucide](https://lucide.dev) (ISC).
