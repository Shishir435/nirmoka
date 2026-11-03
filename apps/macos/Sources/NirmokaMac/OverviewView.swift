import SwiftUI

private struct LargestConsumer: Identifiable {
  let category: StorageCategory
  let consumer: CategoryConsumer

  var id: UInt32 { consumer.id }
}

struct OverviewView: View {
  @EnvironmentObject private var model: AppModel
  let showExplorer: () -> Void
  let showClean: () -> Void
  let showApplications: () -> Void
  let showApplicationInspector: () -> Void
  let showActivity: () -> Void

  private let columns = [GridItem(.flexible(), spacing: 12), GridItem(.flexible(), spacing: 12)]

  private var largestConsumers: [LargestConsumer] {
    guard let categories = model.categories?.categories else { return [] }
    let consumers =
      categories
      .flatMap { category in
        category.consumers.map { LargestConsumer(category: category.category, consumer: $0) }
      }
    return
      consumers
      .filter {
        model.dashboardCategoryFilter == nil || $0.category == model.dashboardCategoryFilter
      }
      .sorted { $0.consumer.totalBytes > $1.consumer.totalBytes }
      .prefix(8)
      .map { $0 }
  }

  private var canReviewCleanup: Bool { model.canClean }

  private var canManageApplications: Bool { model.canListApplications }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 24) {
        pageHeader

        if let message = model.trashStatusMessage {
          HStack(spacing: 12) {
            Label(message, systemImage: "checkmark.circle.fill")
              .font(.callout.weight(.medium))
              .foregroundStyle(NirmokaPalette.ink)
            Spacer()
            Button("View Activity", systemImage: "clock.arrow.circlepath", action: showActivity)
              .buttonStyle(.link)
            Button("Open Trash") { model.openTrash() }.buttonStyle(.link)
            Button("Rescan") { model.rescan() }.disabled(!model.canScan)
          }
          .padding(14)
          .background(NirmokaPalette.accent.opacity(0.08), in: RoundedRectangle(cornerRadius: 12))
        }

        if let summary = model.scanSummary, let breakdown = model.categories {
          capacityCard
          VStack(alignment: .leading, spacing: 6) {
            Text("This scan · \(NirmokaFormat.bytes(summary.totalBytes)) measured")
              .font(.headline)
            Text(summary.rootPath).font(.caption).foregroundStyle(.secondary).textSelection(
              .enabled)
            if summary.readErrors > 0 || summary.excluded > 0 {
              Label(
                "Incomplete coverage · \(summary.readErrors) read issues · \(summary.excluded) skipped entries",
                systemImage: "exclamationmark.triangle"
              )
              .font(.caption).foregroundStyle(.orange)
            }
          }
          HStack(alignment: .top, spacing: 16) {
            VStack(alignment: .leading, spacing: 18) {
              categorySection(breakdown.categories)
              quickActions
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            largestSection
              .frame(maxWidth: .infinity, alignment: .leading)
          }
          scanDetails(summary)
        } else {
          capacityCard
          if model.scannerReady { firstScanCard } else { BackendSetupView() }
          quickActions
        }
      }
      .padding(.horizontal, 30)
      .padding(.top, 28)
      .padding(.bottom, 34)
      .frame(maxWidth: 1100, alignment: .leading)
      .frame(maxWidth: .infinity)
    }
    .background(NirmokaPalette.canvas)
  }

  private var pageHeader: some View {
    HStack(alignment: .top, spacing: 24) {
      VStack(alignment: .leading, spacing: 8) {
        Text("STORAGE OVERVIEW")
          .font(.caption.weight(.bold))
          .tracking(1.4)
          .foregroundStyle(NirmokaPalette.accentDeep)
        Text("See what’s using your storage.")
          .font(.system(size: 30, weight: .semibold, design: .rounded))
          .foregroundStyle(NirmokaPalette.ink)
          .fixedSize(horizontal: false, vertical: true)
        Text("See what is taking room, where it lives, and what deserves a closer look.")
          .font(.callout)
          .foregroundStyle(.secondary)
          .fixedSize(horizontal: false, vertical: true)
      }
    }
  }

  private var capacityCard: some View {
    Group {
      if let volume = model.categories?.volume ?? model.volumeInfo {
        VolumeCard(volume: volume, scanPath: model.scanSummary?.rootPath)
      } else {
        HStack(spacing: 14) {
          Image(systemName: "externaldrive")
            .font(.title2)
            .foregroundStyle(NirmokaPalette.accent)
          VStack(alignment: .leading, spacing: 4) {
            Text("Storage capacity is unavailable")
              .font(.headline)
            Text("You can still inspect the files Nirmoka is able to scan.")
              .font(.callout)
              .foregroundStyle(.secondary)
          }
          Spacer()
        }
        .padding(20)
        .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
      }
    }
  }

  private var firstScanCard: some View {
    HStack(alignment: .top, spacing: 20) {
      ZStack {
        RoundedRectangle(cornerRadius: 14)
          .fill(NirmokaPalette.accent.opacity(0.12))
          .frame(width: 54, height: 54)
        Image(systemName: "folder.badge.questionmark")
          .font(.system(size: 22, weight: .medium))
          .foregroundStyle(NirmokaPalette.accentDeep)
      }

      VStack(alignment: .leading, spacing: 7) {
        Text("Start with the folders you use every day")
          .font(.title3.weight(.semibold))
          .foregroundStyle(NirmokaPalette.ink)
        Text(
          "A Home folder scan can reveal large app support folders, developer data, downloads, and other space that macOS groups together."
        )
        .font(.callout)
        .foregroundStyle(.secondary)
        .fixedSize(horizontal: false, vertical: true)
        HStack(spacing: 7) {
          Image(systemName: "info.circle")
          Text("A large folder is a clue, not a recommendation to delete it.")
        }
        .font(.caption)
        .foregroundStyle(NirmokaPalette.mutedInk)
        .padding(.top, 3)

        if !model.isScanning {
          Button {
            model.scanHome()
          } label: {
            Label("Scan Home Folder", systemImage: "sparkle.magnifyingglass")
          }
          .buttonStyle(.borderedProminent)
          .disabled(!model.canScan)
          .tint(NirmokaPalette.accentDeep)
          .padding(.top, 6)
        } else {
          ProgressView("Scanning your Home folder…")
            .padding(.top, 6)
        }
      }

      Spacer(minLength: 0)
    }
    .padding(22)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 18))
    .overlay {
      RoundedRectangle(cornerRadius: 18)
        .stroke(NirmokaPalette.line, lineWidth: 1)
    }
  }

  private func categorySection(_ categories: [CategorySummary]) -> some View {
    VStack(alignment: .leading, spacing: 12) {
      SectionHeading(
        title: "Where the scanned space lives",
        subtitle: "Select a group to filter the largest items beside it.")

      LazyVGrid(columns: columns, spacing: 12) {
        ForEach(categories) { category in
          CategoryCard(
            category: category,
            selected: model.dashboardCategoryFilter == category.category
          ) {
            model.toggleDashboardCategory(category.category)
          }
        }
      }
    }
  }

  private var largestSection: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack(alignment: .firstTextBaseline) {
        SectionHeading(
          title: model.dashboardCategoryFilter.map { "Largest in \($0.title)" }
            ?? "Largest space users",
          subtitle: model.dashboardCategoryFilter == nil
            ? "Select an item to inspect it. Right-click for actions."
            : "Showing the largest items attributed to this group. Select one to inspect it."
        )
        Spacer()
        if model.dashboardCategoryFilter != nil {
          Button("Show all") { model.toggleDashboardCategory(nil) }
            .buttonStyle(.link)
        }
        if model.scanSummary != nil {
          Button("Explore all", systemImage: "arrow.up.right") {
            showExplorer()
            model.openRootBrowser()
          }
          .buttonStyle(.link)
        }
      }

      if largestConsumers.isEmpty {
        NirmokaEmptyState(
          title: model.dashboardCategoryFilter == nil
            ? "Nothing to rank yet" : "No large items in this group",
          symbol: "externaldrive",
          description: model.dashboardCategoryFilter == nil
            ? "Run a scan to see the largest folders and files."
            : "Try another group or show all space users."
        )
        .frame(minHeight: 150)
        .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
      } else {
        VStack(spacing: 0) {
          ForEach(largestConsumers) { item in
            LargestConsumerRow(
              item: item,
              trashed: model.trashedRowIDs.contains(item.consumer.id),
              canMoveToTrash: model.canMoveToTrash(item.consumer),
              inspect: {
                if item.category == .apps,
                  item.consumer.isDir,
                  URL(fileURLWithPath: item.consumer.path).pathExtension.lowercased() == "app"
                {
                  model.inspectApplication(item.consumer)
                  showApplicationInspector()
                } else {
                  model.inspect(item.consumer)
                  showExplorer()
                }
              },
              open: {
                model.open(item.consumer)
                showExplorer()
              },
              reveal: { model.reveal(item.consumer) },
              moveToTrash: { model.prepareTrash(item.consumer) }
            )
            if let last = largestConsumers.last, item.id != last.id {
              Divider().padding(.leading, 56)
            }
          }
        }
        .padding(.horizontal, 14)
        .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
        .overlay {
          RoundedRectangle(cornerRadius: 16)
            .stroke(NirmokaPalette.line, lineWidth: 1)
        }
      }
    }
  }

  private var quickActions: some View {
    VStack(alignment: .leading, spacing: 10) {
      SectionHeading(title: "Next steps", subtitle: "Review details before you make changes.")

      Button(action: showClean) {
        quickAction(
          title: "Review cleanup",
          subtitle: canReviewCleanup
            ? "See Mole’s current preview before running it."
            : "Set up cleanup to review a plan.",
          symbol: "sparkles",
          tint: NirmokaPalette.accentDeep
        )
      }
      .buttonStyle(.plain)

      Button(action: showApplications) {
        quickAction(
          title: "Manage apps",
          subtitle: canManageApplications
            ? "Inspect app storage or review a removal."
            : "Set up application inventory and removal.",
          symbol: "square.stack.3d.up",
          tint: .blue
        )
      }
      .buttonStyle(.plain)

    }
  }

  private func quickAction(title: String, subtitle: String, symbol: String, tint: Color)
    -> some View
  {
    HStack(spacing: 11) {
      Image(systemName: symbol)
        .font(.system(size: 15, weight: .semibold))
        .foregroundStyle(tint)
        .frame(width: 34, height: 34)
        .background(tint.opacity(0.12), in: RoundedRectangle(cornerRadius: 9))
      VStack(alignment: .leading, spacing: 3) {
        Text(title).font(.subheadline.weight(.semibold)).foregroundStyle(NirmokaPalette.ink)
        Text(subtitle).font(.caption).foregroundStyle(.secondary).fixedSize(
          horizontal: false, vertical: true)
      }
      Spacer(minLength: 4)
      Image(systemName: "chevron.right")
        .font(.caption.weight(.semibold))
        .foregroundStyle(.tertiary)
    }
    .padding(12)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 12))
    .overlay {
      RoundedRectangle(cornerRadius: 12)
        .stroke(NirmokaPalette.line, lineWidth: 1)
    }
    .contentShape(RoundedRectangle(cornerRadius: 12))
  }

  private func scanDetails(_ summary: ScanSummary) -> some View {
    HStack(spacing: 0) {
      ScanMetric(
        title: "Items scanned", value: NirmokaFormat.count(summary.entries), symbol: "doc.on.doc")
      Divider().frame(height: 34)
      ScanMetric(
        title: "Folders", value: NirmokaFormat.count(summary.directories), symbol: "folder")
      Divider().frame(height: 34)
      ScanMetric(
        title: "Read issues", value: NirmokaFormat.count(summary.readErrors),
        symbol: "exclamationmark.circle")
      Divider().frame(height: 34)
      ScanMetric(
        title: "Skipped", value: NirmokaFormat.count(summary.excluded), symbol: "eye.slash")
    }
    .padding(.vertical, 15)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
    .overlay {
      RoundedRectangle(cornerRadius: 16)
        .stroke(NirmokaPalette.line, lineWidth: 1)
    }
  }
}

private struct VolumeCard: View {
  let volume: VolumeInfo
  let scanPath: String?

  private var usedRatio: Double {
    guard volume.totalBytes > 0 else { return 0 }
    return min(1, Double(volume.usedBytes) / Double(volume.totalBytes))
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 18) {
      HStack(alignment: .top) {
        VStack(alignment: .leading, spacing: 5) {
          Label(volume.name, systemImage: "internaldrive.fill")
            .font(.title3.weight(.semibold))
          Text(scanPath.map { "Scanned folder: \($0)" } ?? "Volume capacity")
            .font(.caption)
            .foregroundStyle(.white.opacity(0.72))
            .lineLimit(1)
            .truncationMode(.middle)
        }
        Spacer()
        VStack(alignment: .trailing, spacing: 3) {
          Text(NirmokaFormat.bytes(volume.freeBytes))
            .font(.system(size: 26, weight: .semibold, design: .rounded))
          Text("available")
            .font(.caption.weight(.medium))
            .foregroundStyle(.white.opacity(0.72))
        }
      }

      GeometryReader { geometry in
        ZStack(alignment: .leading) {
          Capsule().fill(.white.opacity(0.16))
          Capsule()
            .fill(Color(red: 0.64, green: 0.91, blue: 0.74))
            .frame(width: geometry.size.width * usedRatio)
        }
      }
      .frame(height: 9)

      HStack {
        Label("\(NirmokaFormat.bytes(volume.usedBytes)) used", systemImage: "circle.fill")
          .labelStyle(CompactLabelStyle())
        Spacer()
        Text("of \(NirmokaFormat.bytes(volume.totalBytes))")
          .foregroundStyle(.white.opacity(0.72))
      }
      .font(.caption.weight(.medium))
    }
    .foregroundStyle(.white)
    .padding(22)
    .background {
      RoundedRectangle(cornerRadius: 18)
        .fill(
          LinearGradient(
            colors: [NirmokaPalette.heroTop, NirmokaPalette.heroBottom],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
          ))
    }
  }
}

private struct CompactLabelStyle: LabelStyle {
  func makeBody(configuration: Configuration) -> some View {
    HStack(spacing: 6) {
      configuration.icon.foregroundStyle(Color(red: 0.64, green: 0.91, blue: 0.74))
      configuration.title
    }
  }
}

private struct CategoryCard: View {
  let category: CategorySummary
  let selected: Bool
  let action: () -> Void

  var body: some View {
    Button(action: action) {
      HStack(spacing: 12) {
        RoundedRectangle(cornerRadius: 3)
          .fill(NirmokaPalette.color(for: category.category))
          .frame(width: 7, height: 40)

        VStack(alignment: .leading, spacing: 5) {
          Text(category.category.title)
            .font(.subheadline.weight(.medium))
            .foregroundStyle(NirmokaPalette.ink)
          Text(NirmokaFormat.percent(category.share) + " of scanned space")
            .font(.caption)
            .foregroundStyle(.secondary)
        }

        Spacer(minLength: 8)

        Text(NirmokaFormat.bytes(category.totalBytes))
          .font(.system(.body, design: .rounded).weight(.semibold))
          .monospacedDigit()
          .foregroundStyle(NirmokaPalette.ink)
      }
      .padding(.horizontal, 14)
      .padding(.vertical, 13)
      .frame(maxWidth: .infinity, alignment: .leading)
      .background(
        selected ? NirmokaPalette.accent.opacity(0.08) : NirmokaPalette.card,
        in: RoundedRectangle(cornerRadius: 14)
      )
      .overlay {
        RoundedRectangle(cornerRadius: 14)
          .stroke(
            selected ? NirmokaPalette.accentDeep : NirmokaPalette.line,
            lineWidth: selected ? 1.5 : 1)
      }
    }
    .buttonStyle(.plain)
    .help(
      selected
        ? "Show all space users" : "Show the largest \(category.category.title.lowercased()) items")
  }
}

private struct LargestConsumerRow: View {
  let item: LargestConsumer
  let trashed: Bool
  let canMoveToTrash: Bool
  let inspect: () -> Void
  let open: () -> Void
  let reveal: () -> Void
  let moveToTrash: () -> Void

  var body: some View {
    Button(action: inspect) {
      HStack(spacing: 13) {
        Image(systemName: item.consumer.isDir ? "folder.fill" : "doc.fill")
          .font(.system(size: 17))
          .foregroundStyle(NirmokaPalette.color(for: item.category))
          .frame(width: 28)

        VStack(alignment: .leading, spacing: 4) {
          HStack(spacing: 7) {
            Text(item.consumer.name)
              .font(.subheadline.weight(.medium))
              .foregroundStyle(NirmokaPalette.ink)
              .lineLimit(1)
            if item.consumer.sizeIsPartial {
              Image(systemName: "exclamationmark.circle.fill")
                .font(.caption)
                .foregroundStyle(.orange)
                .help("This size is a lower bound because some entries could not be read.")
            }
          }
          Text(
            trashed
              ? "In Trash · rescan to update sizes"
              : "\(item.category.title) · \(item.consumer.path)"
          )
          .font(.caption)
          .foregroundStyle(trashed ? NirmokaPalette.mutedInk : .secondary)
          .lineLimit(1)
          .truncationMode(.middle)
        }

        Spacer(minLength: 12)

        Text(NirmokaFormat.bytes(item.consumer.totalBytes))
          .font(.system(.body, design: .rounded).weight(.semibold))
          .monospacedDigit()
          .foregroundStyle(NirmokaPalette.ink)

        Image(systemName: "info.circle")
          .font(.caption)
          .foregroundStyle(.tertiary)
          .padding(.leading, 2)
      }
      .padding(.vertical, 13)
      .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .help("Inspect \(item.consumer.path). Right-click for more actions.")
    .contextMenu {
      Button(action: inspect) {
        Label("Inspect", systemImage: "info.circle")
      }
      Button(action: open) {
        Label(item.consumer.isDir ? "Open Folder" : "Show in Explorer", systemImage: "folder")
      }
      Button(action: reveal) {
        Label("Reveal in Finder", systemImage: "arrow.up.right.square")
      }
      Divider()
      Button(role: .destructive, action: moveToTrash) {
        Label(
          item.consumer.isDir ? "Move Folder to Trash…" : "Move to Trash…", systemImage: "trash")
      }
      .disabled(!canMoveToTrash)
    }
  }
}

struct SectionHeading: View {
  let title: String
  let subtitle: String

  var body: some View {
    VStack(alignment: .leading, spacing: 4) {
      Text(title)
        .font(.title3.weight(.semibold))
        .foregroundStyle(NirmokaPalette.ink)
      Text(subtitle)
        .font(.callout)
        .foregroundStyle(.secondary)
    }
  }
}

private struct ScanProgressCard: View {
  @EnvironmentObject private var model: AppModel

  var body: some View {
    HStack(spacing: 16) {
      ProgressView()
        .controlSize(.large)
      VStack(alignment: .leading, spacing: 6) {
        Text("Mapping your storage…")
          .font(.headline)
        if let progress = model.scanProgress {
          Text("\(NirmokaFormat.count(progress.scanned)) entries · \(progress.currentPath)")
            .font(.caption)
            .foregroundStyle(.secondary)
            .lineLimit(1)
            .truncationMode(.middle)
        } else {
          Text("Large folders may take a little while.")
            .font(.caption)
            .foregroundStyle(.secondary)
        }
      }
      Spacer()
      Button("Stop") { model.cancelScan() }
        .buttonStyle(.bordered)
    }
    .padding(17)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 15))
    .overlay {
      RoundedRectangle(cornerRadius: 15)
        .stroke(NirmokaPalette.line, lineWidth: 1)
    }
  }
}

private struct ScanMetric: View {
  let title: String
  let value: String
  let symbol: String

  var body: some View {
    HStack(spacing: 10) {
      Image(systemName: symbol)
        .foregroundStyle(NirmokaPalette.accentDeep)
      VStack(alignment: .leading, spacing: 2) {
        Text(value).font(.subheadline.weight(.semibold)).monospacedDigit()
        Text(title).font(.caption).foregroundStyle(.secondary)
      }
      Spacer(minLength: 8)
    }
    .frame(maxWidth: .infinity)
    .padding(.horizontal, 14)
  }
}
