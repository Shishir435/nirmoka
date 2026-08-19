# ADR 0028: An application's footprint is what the filesystem says

- Status: accepted
- Date: 2026-08-20

## Context

The approved design puts one number at the centre of every screen: **Docker — 47.2 GB**. Nothing in
the codebase can produce it. Two things can be said today and neither is that number:

- `Docker.app is 1.8 GB`, from the scan tree, which is the bundle and nothing else
- `Mole says "410.9MB"`, a rounded label deliberately kept as a string rather than converted into a
  byte count — see `InstalledApplication` in `dto.rs`

The gap between 1.8 GB and 47.2 GB is `~/Library`. Containers, caches, application support, saved
state, logs, preferences: storage that belongs to an application by every meaning a user has, and to
no application at all by the only meaning the scan tree has, which is the directory it sits in.

Closing that gap needs a key. `CFBundleIdentifier` from `Contents/Info.plist` is the key, and Mole
already publishes `bundle_id` for everything it can address, so scan-derived apps are the only case
that needs the plist read.

The harder question is what the components are called. The design names Docker's as Application,
Containers, **Images**, **Volumes**, Logs & Cache. Images and Volumes are not filesystem facts.
Docker Desktop keeps all of them inside one sparse file,
`~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw` — a file the design's own
drill-down screen shows as a single row. Splitting it into 8.2 GB of images and 4.6 GB of volumes
requires asking the Docker daemon, which means a Docker backend, which is a scanner-shaped
dependency on a program that is not a scanner.

## Decision

**A footprint is the application bundle plus the `~/Library` paths that carry its bundle id.**
Discovery is a fixed list of locations — Application Support, Caches, Containers, Preferences, Saved
Application State, HTTPStorages, WebKit, Logs — each checked for existence before it is reported.
Nothing is inferred from an app's name, only from its identifier.

**Sizes come from the scan tree first and the filesystem second.** If the user scanned `~`, every
Library path is already in the tree and costs nothing to look up. If they scanned `/Applications`,
the Library paths are outside the scanned set and are walked directly. A path whose size is available
from neither is reported as unavailable, not as zero.

**Components are named by where they live, not by what the application keeps there.** Application,
Containers, Caches, Application Support, Logs, Preferences. These are locations macOS defines and
Nirmoka can verify. Images and Volumes are Docker's vocabulary for the contents of a file Nirmoka can
only see the outside of.

**A vendor-named directory is a guess, reported as one.** The identifier only works for
applications that use it. Measured on twelve installed bundles: Docker keeps 1.89 GB under
`~/Library/Containers/com.docker.docker` and the key finds all of it, while Chrome keeps 6.46 GB
under `~/Library/Application Support/Google` and the key finds none of it. Non-sandboxed
applications file storage under a vendor or product name, which is the same word the identifier is
made of — so the candidate names are derived mechanically from `com.google.Chrome` and the bundle's
own name, never from a list somebody maintains.

What is found that way goes into one component, **Possibly related**, and is _excluded from the
footprint total_. It is reported as `related_bytes`, a second number. `~/Library/Application
Support/Google` may hold another Google application's data, and there is no way to tell from the
outside; a total that quietly absorbed it would be confident about a guess. The window styles the
component as uncertain because `StorageComponent::certain` says it is.

**A partial read is a lower bound, said out loud.** `Tree::rollup` sums bytes and propagates
nothing else, so a directory whose own entry read cleanly carries a clean flag over an unreadable
descendant. Completeness is therefore taken from the subtree, not from the node, at both levels —
the bundle and every Library path. A locked cache directory still contributes what was counted, and
the component says the total is a bound rather than a number.

**An opaque container is one row at its real size.** `Docker.raw` is 22 GB of something. Nirmoka
reports 22 GB and its path. It does not guess at the shape inside.

## Consequences

The centre of the product becomes computable, and it is computable without a new backend, a new
`Capabilities` flag, or a widened wire format. `attribution.rs` reads a plist and stats directories.
That is the whole mechanism.

An application's screen can therefore carry two numbers rather than one, and the design's mockups
carry one. That is the cost of being right about Chrome: 1.38 GB attributed and 6.46 GB probably
related is a more useful pair of facts than either 1.38 GB alone, which is wrong by five times, or
7.84 GB stated flatly, which cannot be checked. Where nothing vendor-named is found — Docker,
and every sandboxed application — the second number is zero and the screen shows one number, as
drawn.

The App Inspector shows six component rows where the design shows five, with four names in common.
Someone holding the mockup beside the build sees the same screen and reads different words in two
rows. That is the correct trade: the design's labels are more satisfying and two of them are numbers
no one can check.

This closes off a tempting direction. Per-application storage insight of the kind the design
imagines — this many images, that many stale volumes — is a real product, and it is a product built
out of one integration per application. Nirmoka drives general-purpose disk tools. An app-specific
integration is the first of forty.

What this does not change: `crates/core` gains nothing, because a footprint is a shell concern
assembled from a tree the core already owns. No adapter is asked a new question.
