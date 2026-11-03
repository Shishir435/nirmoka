import SwiftUI

struct AppInspectorView: View {
  @EnvironmentObject private var model: AppModel
  let showExplorer: () -> Void
  let showApplications: () -> Void

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 20) {
        if model.isLoadingAppFootprint {
          ProgressView("Checking this app’s storage locations…")
            .frame(maxWidth: .infinity, minHeight: 260)
            .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
        } else if let footprint = model.appFootprint {
          footprintContent(footprint)
        } else {
          NirmokaEmptyState(
            title: "Choose an app from Storage",
            symbol: "app.dashed",
            description: "Nirmoka combines the app bundle with identified data in your Library."
          )
          .frame(maxWidth: .infinity, minHeight: 260)
          .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
        }
      }
      .padding(28)
      .frame(maxWidth: 1000, alignment: .leading)
      .frame(maxWidth: .infinity)
    }
    .background(NirmokaPalette.canvas)
  }

  private func footprintContent(_ footprint: AppFootprint) -> some View {
    VStack(alignment: .leading, spacing: 20) {
      VStack(alignment: .leading, spacing: 8) {
        Text("APPLICATION FOOTPRINT")
          .font(.caption.weight(.bold))
          .tracking(1.2)
          .foregroundStyle(NirmokaPalette.accentDeep)
        HStack(alignment: .top) {
          VStack(alignment: .leading, spacing: 6) {
            Text(footprint.name)
              .font(.system(size: 30, weight: .semibold, design: .rounded))
            Text(footprint.path)
              .font(.callout)
              .foregroundStyle(.secondary)
              .textSelection(.enabled)
              .lineLimit(2)
              .truncationMode(.middle)
            if let bundleId = footprint.bundleId {
              Text(bundleId)
                .font(.caption.monospaced())
                .foregroundStyle(.tertiary)
            }
          }
          Spacer(minLength: 10)
          Menu {
            if footprint.scanId != 0 {
              Button("Browse app files", systemImage: "folder") {
                model.openApplicationInBrowser()
                showExplorer()
              }
            }
            Button("Reveal in Finder", systemImage: "arrow.up.right.square") {
              model.revealApplicationInFinder()
            }
          } label: {
            Label("Actions", systemImage: "ellipsis.circle")
          }
          .menuStyle(.borderlessButton)
          .fixedSize()
        }
      }

      HStack {
        Button("Reveal in Finder", systemImage: "folder") { model.revealApplicationInFinder() }
        Spacer()
        if let application = model.inspectedApplication, model.canUninstallApplications {
          Button("Review uninstall…", systemImage: "trash") {
            model.selectForRemoval(application)
            showApplications()
            model.previewUninstall()
          }.disabled(model.isExecutingUninstall)
        } else {
          Button("Find removal options…") { showApplications() }
        }
      }

      HStack(spacing: 12) {
        FootprintMetricCard(
          title: "Attributed total",
          value: footprint.components.contains(where: { $0.certain && !$0.complete })
            ? "At least \(NirmokaFormat.bytes(footprint.totalBytes))"
            : NirmokaFormat.bytes(footprint.totalBytes),
          symbol: "externaldrive"
        )
        FootprintMetricCard(
          title: "App folder",
          value: footprint.components.first(where: { $0.label == "Application" }).map {
            NirmokaFormat.bytes($0.totalBytes)
          } ?? "Not reported",
          symbol: "app"
        )
        FootprintMetricCard(
          title: "Library locations",
          value: NirmokaFormat.count(
            UInt64(
              footprint.components.filter { $0.label != "Application" && $0.certain }.flatMap(
                \.paths
              ).count)),
          symbol: "folder"
        )
      }

      if let lastUsedMs = footprint.lastUsedMs {
        Label(
          "Last opened \(Date(timeIntervalSince1970: Double(lastUsedMs) / 1_000).formatted(date: .abbreviated, time: .shortened))",
          systemImage: "clock"
        )
        .font(.caption)
        .foregroundStyle(.secondary)
      }

      if footprint.unmeasuredPaths > 0 || footprint.bundleId == nil || footprint.relatedBytes > 0 {
        VStack(alignment: .leading, spacing: 8) {
          Label("How to read this total", systemImage: "info.circle")
            .font(.subheadline.weight(.semibold))
          if footprint.unmeasuredPaths > 0 {
            Text(
              "At least \(NirmokaFormat.count(UInt64(footprint.unmeasuredPaths))) location(s) could not be measured, so the total is a lower bound."
            )
          }
          if footprint.bundleId == nil {
            Text(
              "This app has no readable bundle identifier. The total includes only its app folder.")
          }
          if footprint.relatedBytes > 0 {
            Text(
              "\(NirmokaFormat.bytes(footprint.relatedBytes)) of related data matched by vendor name. It is shown separately and is not included in the attributed total."
            )
          }
        }
        .font(.caption)
        .foregroundStyle(.secondary)
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 12))
      }

      VStack(alignment: .leading, spacing: 10) {
        SectionHeading(
          title: "Where its data lives",
          subtitle: "Library locations are matched by bundle identifier and shown separately."
        )
        ForEach(footprint.components.sorted { $0.totalBytes > $1.totalBytes }) { component in
          DisclosureGroup {
            VStack(alignment: .leading, spacing: 9) {
              ForEach(component.paths) { path in
                HStack(alignment: .top, spacing: 10) {
                  VStack(alignment: .leading, spacing: 4) {
                    Text(path.path)
                      .font(.caption.monospaced())
                      .textSelection(.enabled)
                      .lineLimit(2)
                      .truncationMode(.middle)
                    Text(pathDescription(path))
                      .font(.caption2)
                      .foregroundStyle(path.totalBytes == nil ? .orange : .secondary)
                  }
                  Spacer(minLength: 8)
                  Button {
                    model.revealPath(path.path)
                  } label: {
                    Image(systemName: "arrow.up.right.square")
                  }
                  .buttonStyle(.borderless).help("Reveal this location in Finder")
                  Text(pathSize(path))
                    .font(.caption.weight(.medium).monospacedDigit())
                    .foregroundStyle(.secondary)
                }
              }
            }
            .padding(.vertical, 8)
          } label: {
            HStack(spacing: 8) {
              Image(systemName: component.label == "Application" ? "app" : "folder")
                .foregroundStyle(
                  component.label == "Application"
                    ? NirmokaPalette.accentDeep : NirmokaPalette.mutedInk)
              Text(component.label)
                .font(.subheadline.weight(.semibold))
              if !component.certain {
                Text("RELATED")
                  .font(.caption2.weight(.bold))
                  .foregroundStyle(.orange)
              }
              Spacer()
              Text(
                component.complete
                  ? NirmokaFormat.bytes(component.totalBytes)
                  : "At least \(NirmokaFormat.bytes(component.totalBytes))"
              )
              .font(.callout.weight(.medium).monospacedDigit())
              .foregroundStyle(.secondary)
            }
          }
          .padding(14)
          .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 12))
          .overlay {
            RoundedRectangle(cornerRadius: 12)
              .stroke(NirmokaPalette.line, lineWidth: 1)
          }
        }
      }
    }
  }

  private func pathSize(_ path: FootprintPath) -> String {
    guard let size = path.totalBytes else { return "Unavailable" }
    return path.complete ? NirmokaFormat.bytes(size) : "At least \(NirmokaFormat.bytes(size))"
  }

  private func pathDescription(_ path: FootprintPath) -> String {
    let source: String
    switch path.source {
    case .scan: source = "Measured during scan"
    case .filesystem: source = "Measured from disk"
    case .unavailable: source = "Could not be measured"
    }
    return path.complete ? source : "\(source) · some contents could not be read"
  }
}

private struct FootprintMetricCard: View {
  let title: String
  let value: String
  let symbol: String

  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      Label(title, systemImage: symbol)
        .font(.caption.weight(.medium))
        .foregroundStyle(.secondary)
      Text(value)
        .font(.title3.weight(.semibold).monospacedDigit())
        .lineLimit(1)
        .minimumScaleFactor(0.7)
    }
    .padding(15)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 12))
  }
}
