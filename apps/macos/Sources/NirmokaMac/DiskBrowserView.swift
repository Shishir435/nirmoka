import SwiftUI

struct DiskBrowserView: View {
  @EnvironmentObject private var model: AppModel
  @FocusState private var listFocused: Bool

  var body: some View {
    VStack(spacing: 0) {
      browserHeader
      Divider()

      if let page = model.rowPage {
        HStack(spacing: 0) {
          fileList(page)
            .disabled(model.isScanning || model.isLoadingRows)
            .overlay {
              if model.isLoadingRows {
                ProgressView("Loading folder…").padding().background(
                  .regularMaterial, in: RoundedRectangle(cornerRadius: 10))
              }
            }
          Divider()
          InspectorView()
            .frame(width: 300)
        }
      } else {
        NirmokaEmptyState(
          title: "Scan a folder to explore it",
          symbol: "folder.badge.magnifyingglass",
          description:
            "Choose Scan in the toolbar, then select a folder. Scanning only reads your files."
        )
        .frame(maxWidth: .infinity, maxHeight: .infinity)
      }
    }
    .background(NirmokaPalette.canvas)
  }

  private var browserHeader: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack {
        VStack(alignment: .leading, spacing: 4) {
          Text("EXPLORE")
            .font(.caption.weight(.bold))
            .tracking(1.4)
            .foregroundStyle(NirmokaPalette.accentDeep)
          Text("Inspect the details")
            .font(.system(size: 25, weight: .semibold, design: .rounded))
            .foregroundStyle(NirmokaPalette.ink)
        }

        Spacer()

        Button {
          model.goBackInBrowser()
        } label: {
          Image(systemName: "chevron.left")
        }
        .disabled(!model.browserHistory.canGoBack || model.isLoadingRows).help("Back")
        Button {
          model.goForwardInBrowser()
        } label: {
          Image(systemName: "chevron.right")
        }
        .disabled(!model.browserHistory.canGoForward || model.isLoadingRows).help("Forward")
        Picker(
          "Sort", selection: Binding(get: { model.browserSort }, set: { model.setBrowserSort($0) })
        ) {
          Text("Largest first").tag(SortOrder.largestFirst)
          Text("Smallest first").tag(SortOrder.smallestFirst)
          Text("Name A–Z").tag(SortOrder.nameAscending)
          Text("Name Z–A").tag(SortOrder.nameDescending)
        }.frame(width: 180).disabled(model.isLoadingRows)
        if let row = model.selectedRow, row.kind == .directory {
          Button("Open folder", systemImage: "folder") { model.open(row) }
            .keyboardShortcut(.downArrow, modifiers: .command)
        }
        Button {
          model.goUpInBrowser()
        } label: {
          Label("Up", systemImage: "arrow.up")
        }
        .buttonStyle(.bordered)
        .disabled(model.rowPage?.ancestors.isEmpty ?? true)
        .help("Go to the parent folder")
        .keyboardShortcut(.upArrow, modifiers: .command)
      }

      if let page = model.rowPage {
        ScrollView(.horizontal, showsIndicators: false) {
          HStack(spacing: 7) {
            ForEach(page.ancestors) { crumb in
              Button(crumb.name) { model.open(crumb) }
                .buttonStyle(.link)
              Image(systemName: "chevron.right")
                .font(.system(size: 9, weight: .bold))
                .foregroundStyle(.tertiary)
            }
            Text(page.name)
              .font(.caption.weight(.semibold))
              .foregroundStyle(NirmokaPalette.ink)
          }
        }
        .font(.caption)
        .padding(.vertical, 1)
      }
    }
    .padding(.horizontal, 24)
    .padding(.vertical, 18)
  }

  private func fileList(_ page: RowPage) -> some View {
    VStack(spacing: 0) {
      HStack {
        Text("Name")
          .frame(maxWidth: .infinity, alignment: .leading)
        Text("Right-click for actions")
          .font(.caption2)
          .padding(.trailing, 12)
        Text("On disk")
          .frame(width: 115, alignment: .trailing)
      }
      .font(.caption.weight(.semibold))
      .foregroundStyle(.secondary)
      .padding(.horizontal, 18)
      .padding(.vertical, 10)
      .background(Color.black.opacity(0.025))

      if model.isPreparingTrash || model.isMovingToTrash {
        Label(
          model.isPreparingTrash ? "Checking the selected path…" : "Moving item to Trash…",
          systemImage: model.isPreparingTrash ? "checkmark.shield" : "trash"
        )
        .font(.caption)
        .foregroundStyle(.secondary)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 18)
        .padding(.vertical, 8)
      } else if let message = model.trashStatusMessage {
        Label(message, systemImage: "checkmark.circle.fill")
          .font(.caption)
          .foregroundStyle(NirmokaPalette.mutedInk)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.horizontal, 18)
          .padding(.vertical, 8)
      }

      if page.readError {
        NirmokaEmptyState(
          title: "This folder could not be read",
          symbol: "lock.fill",
          description:
            "macOS did not allow this folder to be read. Check its permissions in Finder. If access is needed, enable Full Disk Access for Nirmoka in System Settings, then rescan."
        )
        .frame(maxWidth: .infinity, maxHeight: .infinity)
      } else if page.rows.isEmpty {
        NirmokaEmptyState(
          title: "This folder is empty",
          symbol: "folder",
          description: "There are no entries to show here."
        )
        .frame(maxWidth: .infinity, maxHeight: .infinity)
      } else {
        List(selection: Binding(get: { model.selectedRowID }, set: { model.selectRow($0) })) {
          ForEach(page.rows) { row in
            DiskRowView(
              row: row,
              selected: row.id == model.selectedRowID || row.id == model.selectedConsumer?.id,
              trashed: model.trashedRowIDs.contains(row.id),
              canMoveToTrash: model.canMoveToTrash(row),
              select: {
                model.selectRow(row.id)
                listFocused = true
              },
              open: { model.open(row) },
              reveal: { model.reveal(row) },
              moveToTrash: { model.prepareTrash(row) }
            )
            .tag(row.id)
            .listRowInsets(EdgeInsets(top: 0, leading: 0, bottom: 0, trailing: 0))
          }
        }
        .listStyle(.inset)
        .focused($listFocused)
      }

      Divider()
      HStack {
        Text(
          page.total == 0
            ? "0 items"
            : "\(page.offset + 1)–\(page.offset + UInt32(page.rows.count)) of \(page.total) items"
        )
        .foregroundStyle(.secondary)
        Spacer()
        Button {
          model.previousRowsPage()
        } label: {
          Image(systemName: "chevron.left")
        }
        .disabled(page.offset == 0)
        .help("Previous page")
        Button {
          model.nextRowsPage()
        } label: {
          Image(systemName: "chevron.right")
        }
        .disabled(page.offset + UInt32(page.rows.count) >= page.total)
        .help("Next page")
      }
      .buttonStyle(.borderless)
      .font(.caption)
      .padding(.horizontal, 16)
      .padding(.vertical, 10)
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
  }
}

private struct DiskRowView: View {
  let row: DiskRow
  let selected: Bool
  let trashed: Bool
  let canMoveToTrash: Bool
  let select: () -> Void
  let open: () -> Void
  let reveal: () -> Void
  let moveToTrash: () -> Void

  var body: some View {
    Button(action: select) {
      HStack(spacing: 10) {
        Image(systemName: symbol)
          .font(.system(size: 15))
          .foregroundStyle(
            row.kind == .directory ? NirmokaPalette.accentDeep : NirmokaPalette.mutedInk
          )
          .frame(width: 23)

        VStack(alignment: .leading, spacing: 4) {
          HStack(spacing: 6) {
            Text(row.name)
              .font(.subheadline.weight(selected ? .semibold : .regular))
              .lineLimit(1)
            if row.readError || row.excluded {
              Image(systemName: "exclamationmark.triangle.fill")
                .font(.caption2)
                .foregroundStyle(.orange)
                .help("Some of this item could not be inspected.")
            }
          }
          Text(trashed ? "In Trash · rescan to update sizes" : detail)
            .font(.caption2)
            .foregroundStyle(trashed ? NirmokaPalette.mutedInk : NirmokaPalette.ink.opacity(0.65))
            .lineLimit(1)
        }

        Spacer(minLength: 8)

        Text(NirmokaFormat.bytes(row.totalBytes))
          .font(.system(.caption, design: .rounded).weight(.medium))
          .monospacedDigit()
          .foregroundStyle(NirmokaPalette.ink)
          .frame(width: 108, alignment: .trailing)

        if row.kind == .directory {
          Image(systemName: "chevron.right")
            .font(.caption.weight(.semibold))
            .foregroundStyle(.tertiary)
        } else {
          Color.clear.frame(width: 20)
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
      .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .simultaneousGesture(
      TapGesture(count: 2)
        .onEnded {
          guard row.kind == .directory else { return }
          open()
        }
    )
    .accessibilityHint(
      row.kind == .directory
        ? "Single-click to inspect. Double-click to open this folder. Right-click for actions."
        : "Click to inspect this item. Right-click for actions."
    )
    .contextMenu {
      Button(action: select) {
        Label("Inspect", systemImage: "info.circle")
      }
      Button(action: reveal) {
        Label("Reveal in Finder", systemImage: "folder")
      }
      Divider()
      Button(role: .destructive, action: moveToTrash) {
        Label(
          row.kind == .directory ? "Move Folder to Trash…" : "Move to Trash…",
          systemImage: "trash"
        )
      }
      .disabled(!canMoveToTrash)
    }
    .padding(.horizontal, 10)
    .padding(.vertical, 7)
    .background(
      selected ? NirmokaPalette.accent.opacity(0.12) : .clear, in: RoundedRectangle(cornerRadius: 8)
    )
  }

  private var symbol: String {
    switch row.kind {
    case .directory: "folder.fill"
    case .file: "doc"
    case .symlink: "arrow.turn.up.right"
    case .other: "questionmark.folder"
    }
  }

  private var detail: String {
    if row.kind == .directory { return "\(NirmokaFormat.count(UInt64(row.childCount))) items" }
    if row.hardlink { return "Shared file link" }
    return row.kind.rawValue.capitalized
  }
}

private struct InspectorView: View {
  @EnvironmentObject private var model: AppModel

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Text("INSPECTOR")
        .font(.caption.weight(.bold))
        .tracking(1.2)
        .foregroundStyle(.secondary)
        .padding(.bottom, 16)

      if let row = model.selectedRow {
        selectedItem(row)
      } else if let consumer = model.selectedConsumer {
        selectedConsumer(consumer)
      } else {
        emptyInspector
      }
      Spacer(minLength: 0)
    }
    .padding(20)
    .frame(maxHeight: .infinity, alignment: .topLeading)
    .background(Color.black.opacity(0.018))
  }

  private func selectedItem(_ row: DiskRow) -> some View {
    VStack(alignment: .leading, spacing: 18) {
      VStack(alignment: .leading, spacing: 8) {
        Image(systemName: row.kind == .directory ? "folder.fill" : "doc.fill")
          .font(.system(size: 24))
          .foregroundStyle(NirmokaPalette.accentDeep)
        Text(row.name)
          .font(.title3.weight(.semibold))
          .foregroundStyle(NirmokaPalette.ink)
          .textSelection(.enabled)
          .lineLimit(2)
      }

      VStack(alignment: .leading, spacing: 10) {
        InspectorValue(label: "On disk", value: NirmokaFormat.bytes(row.totalBytes))
        if row.apparentBytes != row.totalBytes {
          InspectorValue(label: "Logical size", value: NirmokaFormat.bytes(row.apparentBytes))
        }
        InspectorValue(label: "Kind", value: row.kind.rawValue.capitalized)
        InspectorValue(label: "Inside", value: model.rowPage?.path ?? "—")
      }

      if row.readError || row.excluded || row.hardlink {
        VStack(alignment: .leading, spacing: 8) {
          if row.readError || row.excluded {
            Label("Size may be incomplete", systemImage: "exclamationmark.triangle.fill")
              .font(.caption.weight(.semibold))
              .foregroundStyle(.orange)
            Text("Some entries could not be read. Treat this total as a lower bound.")
              .font(.caption)
              .foregroundStyle(.secondary)
          }
          if row.hardlink {
            Label("Shared file data", systemImage: "link")
              .font(.caption.weight(.semibold))
              .foregroundStyle(.secondary)
            Text("This entry may share its disk data with another path.")
              .font(.caption)
              .foregroundStyle(.secondary)
          }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.orange.opacity(0.08), in: RoundedRectangle(cornerRadius: 10))
      }

      Button {
        model.revealSelectedRow()
      } label: {
        Label("Reveal in Finder", systemImage: "folder")
          .frame(maxWidth: .infinity)
      }
      .buttonStyle(.bordered)

      if row.kind == .directory {
        Button("Open folder", systemImage: "folder") { model.open(row) }
          .buttonStyle(.bordered)
      }
      Button(role: .destructive) {
        model.prepareTrash(row)
      } label: {
        Label("Move to Trash…", systemImage: "trash")
      }
      .buttonStyle(.bordered)
      .disabled(!model.canMoveToTrash(row))
      .keyboardShortcut(.delete, modifiers: .command)

      Text(
        "Size alone doesn’t tell you whether an item is safe to remove. Check what it belongs to first."
      )
      .font(.caption)
      .foregroundStyle(.secondary)
      .fixedSize(horizontal: false, vertical: true)
    }
  }

  private func selectedConsumer(_ consumer: CategoryConsumer) -> some View {
    VStack(alignment: .leading, spacing: 18) {
      VStack(alignment: .leading, spacing: 8) {
        Image(systemName: consumer.isDir ? "folder.fill" : "doc.fill")
          .font(.system(size: 24))
          .foregroundStyle(NirmokaPalette.accentDeep)
        Text(consumer.name)
          .font(.title3.weight(.semibold))
          .foregroundStyle(NirmokaPalette.ink)
          .textSelection(.enabled)
          .lineLimit(2)
      }

      VStack(alignment: .leading, spacing: 10) {
        InspectorValue(label: "On disk", value: NirmokaFormat.bytes(consumer.totalBytes))
        InspectorValue(label: "Kind", value: consumer.isDir ? "Folder" : "File")
        InspectorValue(label: "Location", value: consumer.path)
      }

      if consumer.sizeIsPartial {
        Label("Size may be incomplete", systemImage: "exclamationmark.triangle.fill")
          .font(.caption.weight(.semibold))
          .foregroundStyle(.orange)
        Text("Some entries could not be read. Treat this total as a lower bound.")
          .font(.caption)
          .foregroundStyle(.secondary)
      }

      Button {
        model.open(consumer)
      } label: {
        Label(consumer.isDir ? "Open Folder" : "Show in Explorer", systemImage: "folder")
          .frame(maxWidth: .infinity)
      }
      .buttonStyle(.bordered)

      Button {
        model.reveal(consumer)
      } label: {
        Label("Reveal in Finder", systemImage: "arrow.up.right.square")
          .frame(maxWidth: .infinity)
      }
      .buttonStyle(.bordered)

      Button(role: .destructive) {
        model.prepareTrash(consumer)
      } label: {
        Label(consumer.isDir ? "Move Folder to Trash…" : "Move to Trash…", systemImage: "trash")
          .frame(maxWidth: .infinity)
      }
      .buttonStyle(.bordered)
      .disabled(!model.canMoveToTrash(consumer))

      if model.trashedRowIDs.contains(consumer.id) {
        Label("In Trash · rescan to update sizes", systemImage: "checkmark.circle.fill")
          .font(.caption)
          .foregroundStyle(NirmokaPalette.mutedInk)
      }

      Text(
        "A large size is a clue, not proof that an item is safe to remove. Moving it to Trash keeps it recoverable."
      )
      .font(.caption)
      .foregroundStyle(.secondary)
      .fixedSize(horizontal: false, vertical: true)
    }
  }

  private var emptyInspector: some View {
    VStack(alignment: .leading, spacing: 11) {
      Image(systemName: "cursorarrow.click")
        .font(.title2)
        .foregroundStyle(NirmokaPalette.accentDeep)
      Text("Select something to inspect")
        .font(.headline)
        .foregroundStyle(NirmokaPalette.ink)
      Text("Single-click an item to see its details. Double-click a folder to open it.")
        .font(.callout)
        .foregroundStyle(.secondary)
        .fixedSize(horizontal: false, vertical: true)
    }
    .padding(.top, 8)
  }
}

private struct InspectorValue: View {
  let label: String
  let value: String

  var body: some View {
    VStack(alignment: .leading, spacing: 4) {
      Text(label.uppercased())
        .font(.caption2.weight(.semibold))
        .tracking(0.7)
        .foregroundStyle(.secondary)
      Text(value)
        .font(.caption)
        .foregroundStyle(NirmokaPalette.ink)
        .textSelection(.enabled)
        .lineLimit(3)
        .truncationMode(.middle)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
  }
}
