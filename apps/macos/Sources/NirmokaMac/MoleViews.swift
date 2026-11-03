import SwiftUI

struct OptimizeView: View {
  @EnvironmentObject private var model: AppModel

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 20) {
        pageHeader

        if model.isLoadingOptimizePreview {
          HStack(spacing: 12) {
            ProgressView()
            Text("Mole is checking maintenance actions…")
              .font(.headline)
          }
          .padding(18)
          .frame(maxWidth: .infinity, alignment: .leading)
          .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 14))
        } else if let result = model.optimizeResult {
          operationCard(result)
        } else if let preview = model.optimizePreview {
          previewCard(preview)
        } else {
          NirmokaEmptyState(
            title: "Review Mole’s maintenance plan",
            symbol: "gearshape.2",
            description:
              "Mole can refresh system caches and services and repair supported configuration issues. See its dry-run output before approving any changes."
          )
          .frame(maxWidth: .infinity)
          .padding(20)
          .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
        }
      }
      .padding(28)
      .frame(maxWidth: 1100, alignment: .leading)
      .frame(maxWidth: .infinity)
    }
    .background(NirmokaPalette.canvas)
    .confirmationDialog(
      "Run Mole optimization?",
      isPresented: $model.isOptimizeConfirmationPresented,
      titleVisibility: .visible
    ) {
      Button("Run Optimization", role: .destructive) { model.confirmOptimize() }
      Button("Cancel", role: .cancel) {}
    } message: {
      if let preparation = model.optimizePreparation {
        Text(preparation.warning)
      }
    }
  }

  private var pageHeader: some View {
    HStack(alignment: .top, spacing: 18) {
      VStack(alignment: .leading, spacing: 6) {
        Text("OPTIMIZE")
          .font(.caption.weight(.bold))
          .tracking(1.2)
          .foregroundStyle(NirmokaPalette.accentDeep)
        Text("A healthier, more responsive Mac")
          .font(.system(size: 28, weight: .semibold, design: .rounded))
        Text(
          "Mole checks its supported maintenance actions. Review the exact output, then choose whether to run them."
        )
        .font(.callout)
        .foregroundStyle(.secondary)
      }
      Spacer()
      if model.isExecutingOptimize {
        Button("Stop", systemImage: "stop.circle") { model.cancelOptimize() }
          .buttonStyle(.bordered)
      } else if model.isLoadingOptimizePreview {
        Button("Cancel Preview", systemImage: "xmark.circle") { model.cancelOptimizePreview() }
          .buttonStyle(.bordered)
      } else {
        Button("Preview", systemImage: "arrow.clockwise") { model.loadOptimizePreview() }
          .buttonStyle(.borderedProminent)
      }
    }
  }

  private func previewCard(_ preview: OptimizePreview) -> some View {
    VStack(alignment: .leading, spacing: 14) {
      HStack(spacing: 12) {
        MetricCard(
          title: "Actions", value: NirmokaFormat.count(preview.actionCount), symbol: "checklist")
        MetricCard(title: "Backend", value: "Mole \(preview.backendVersion)", symbol: "terminal")
      }

      SectionHeading(
        title: "Mole’s maintenance plan",
        subtitle:
          "No changes are made by this preview. Review each group before choosing to run it.")
      LazyVGrid(
        columns: [GridItem(.flexible()), GridItem(.flexible())], alignment: .leading, spacing: 12
      ) {
        ForEach(preview.sections) { section in
          VStack(alignment: .leading, spacing: 10) {
            Text(section.name)
              .font(.subheadline.weight(.semibold))
            ForEach(section.items) { item in
              HStack(alignment: .top, spacing: 8) {
                Image(systemName: optimizeItemSymbol(item.kind))
                  .foregroundStyle(optimizeItemColor(item.kind))
                  .frame(width: 14)
                Text(item.text)
                  .font(.caption)
                  .foregroundStyle(.secondary)
                  .fixedSize(horizontal: false, vertical: true)
              }
            }
          }
          .padding(14)
          .frame(maxWidth: .infinity, alignment: .leading)
          .background(Color.black.opacity(0.035), in: RoundedRectangle(cornerRadius: 12))
        }
      }

      DisclosureGroup("Show Mole’s full output") {
        ScrollView([.vertical, .horizontal]) {
          Text(preview.transcript)
            .font(.system(.caption, design: .monospaced))
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(12)
        }
        .frame(maxHeight: 300)
      }
      .padding(12)
      .background(Color.black.opacity(0.025), in: RoundedRectangle(cornerRadius: 10))

      HStack {
        Label("Mole applies the approved maintenance steps.", systemImage: "info.circle")
          .font(.caption)
          .foregroundStyle(.secondary)
        Spacer()
        Button("Run Reviewed Plan", systemImage: "gearshape.2") { model.prepareOptimize() }
          .buttonStyle(.borderedProminent)
          .disabled(preview.actionCount == 0 || model.isExecutingOptimize)
      }
    }
    .padding(20)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
  }

  private func optimizeItemSymbol(_ kind: OptimizeItemKind) -> String {
    switch kind {
    case .action: "arrow.triangle.2.circlepath"
    case .complete: "checkmark.circle"
    case .review: "eye"
    case .note: "info.circle"
    }
  }

  private func optimizeItemColor(_ kind: OptimizeItemKind) -> Color {
    switch kind {
    case .action: NirmokaPalette.accentDeep
    case .complete: NirmokaPalette.accent
    case .review: .orange
    case .note: .secondary
    }
  }

  private func operationCard(_ operation: OptimizeOperation) -> some View {
    VStack(alignment: .leading, spacing: 12) {
      Label(
        completionText(operation.completion), systemImage: completionSymbol(operation.completion)
      )
      .font(.title3.weight(.semibold))
      .foregroundStyle(operation.completion == .finished ? NirmokaPalette.accent : .orange)
      Text("Mole \(operation.backendVersion) reported this result.")
        .foregroundStyle(.secondary)
      if !operation.transcript.isEmpty {
        ScrollView([.vertical, .horizontal]) {
          Text(operation.transcript)
            .font(.system(.caption, design: .monospaced))
            .textSelection(.enabled)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(14)
        }
        .frame(maxHeight: 360)
        .background(Color.black.opacity(0.045), in: RoundedRectangle(cornerRadius: 12))
      }
      Button("Review a Fresh Plan", systemImage: "arrow.clockwise") { model.loadOptimizePreview() }
        .buttonStyle(.bordered)
    }
    .padding(20)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
  }

  private func completionText(_ completion: OptimizeCompletion) -> String {
    switch completion {
    case .finished: "Optimization finished"
    case .cancelled: "Optimization stopped"
    case .failed: "Optimization ended with an error"
    }
  }

  private func completionSymbol(_ completion: OptimizeCompletion) -> String {
    switch completion {
    case .finished: "checkmark.circle.fill"
    case .cancelled: "stop.circle.fill"
    case .failed: "exclamationmark.triangle.fill"
    }
  }
}

struct CleanView: View {
  @EnvironmentObject private var model: AppModel

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 20) {
        pageHeader

        if !model.canClean && !model.isCheckingBackends {
          BackendSetupView(scanning: false)
        } else if model.isExecutingCleanup {
          ProgressView("Cleanup is running…")
          Text(
            "Stopping cannot restore anything already removed. Cleaned files may not be recoverable."
          ).font(.callout).foregroundStyle(.secondary)
          Button("Stop cleanup") { model.cancelCleanup() }
        } else if model.isLoadingCleanupPreview {
          progressCard
        } else if let result = model.cleanupResult {
          operationCard(result)
        } else if let preview = model.cleanupPreview {
          previewContent(preview)
        } else {
          NirmokaEmptyState(
            title: "Review cleanup candidates",
            symbol: "sparkles",
            description:
              "Generate a preview to see current cleanup candidates. Nothing is removed during the preview."
          )
          .frame(maxWidth: .infinity)
          .padding(20)
          .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
        }
      }
      .padding(28)
      .frame(maxWidth: 1100, alignment: .leading)
      .frame(maxWidth: .infinity)
    }
    .background(NirmokaPalette.canvas)

    .confirmationDialog(
      "Run Mole cleanup?",
      isPresented: $model.isCleanupConfirmationPresented,
      titleVisibility: .visible
    ) {
      Button("Run Cleanup", role: .destructive) { model.confirmCleanup() }
      Button("Cancel", role: .cancel) {}
    } message: {
      if let preparation = model.cleanupPreparation {
        Text(
          "\(preparation.warning) Mole will discover eligible items again when the run starts. Cleaned files may not be recoverable."
        )
      }
    }
  }

  private var pageHeader: some View {
    HStack(alignment: .top, spacing: 18) {
      VStack(alignment: .leading, spacing: 6) {
        Text("CLEAN")
          .font(.caption.weight(.bold))
          .tracking(1.2)
          .foregroundStyle(NirmokaPalette.accentDeep)
        Text("Review before you clean")
          .font(.system(size: 28, weight: .semibold, design: .rounded))
        Text("Mole chooses what is eligible. Nirmoka adds no cleanup paths of its own.")
          .font(.callout)
          .foregroundStyle(.secondary)
      }
      Spacer()
      if model.isExecutingCleanup {
        Button("Stop", systemImage: "stop.circle") { model.cancelCleanup() }
          .buttonStyle(.bordered)
      } else if model.isLoadingCleanupPreview {
        Button("Cancel Preview", systemImage: "xmark.circle") { model.cancelCleanupPreview() }
          .buttonStyle(.bordered)
      } else {
        Button("Refresh Preview", systemImage: "arrow.clockwise") { model.loadCleanupPreview() }
          .buttonStyle(.borderedProminent)
      }
    }
  }

  private var progressCard: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack(spacing: 10) {
        ProgressView()
        Text("Mole is checking cleanup candidates…")
          .font(.headline)
      }
      ForEach(model.cleanupProgress.suffix(8)) { line in
        HStack(spacing: 8) {
          Image(systemName: progressSymbol(line.kind))
            .foregroundStyle(.secondary)
          Text(line.text)
            .lineLimit(1)
            .truncationMode(.middle)
        }
        .font(.caption)
      }
    }
    .padding(18)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 14))
  }

  private func previewContent(_ preview: CleanupPreview) -> some View {
    VStack(alignment: .leading, spacing: 16) {
      HStack(spacing: 12) {
        MetricCard(
          title: "Potential cleanup", value: preview.potentialCleanup ?? "Not reported",
          symbol: "sparkles")
        MetricCard(
          title: "Items", value: NirmokaFormat.count(preview.totalItems),
          symbol: "doc.text.magnifyingglass")
        MetricCard(title: "Mole", value: preview.backendVersion, symbol: "terminal")
      }

      Text(
        "Mole checks candidates again when cleanup starts. Results may differ from this preview, and cleaned files may not be recoverable."
      )
      .font(.callout).foregroundStyle(.secondary)
      Label(cleanupScopeText(preview.systemScope), systemImage: "info.circle")
        .font(.callout)
        .foregroundStyle(.secondary)
        .padding(.horizontal, 2)

      if !preview.warnings.isEmpty {
        warningsCard(preview.warnings)
      }

      if preview.categories.isEmpty {
        NirmokaEmptyState(
          title: "Nothing to clean", symbol: "checkmark.circle",
          description: "Mole found no cleanup candidates in this preview."
        )
        .frame(maxWidth: .infinity)
        .padding(18)
        .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 14))
      } else {
        VStack(alignment: .leading, spacing: 8) {
          SectionHeading(
            title: "Mole’s cleanup preview",
            subtitle: "These are the paths Mole reported. Execution performs a fresh discovery.")
          ForEach(preview.categories) { category in
            DisclosureGroup {
              VStack(alignment: .leading, spacing: 8) {
                ForEach(category.items) { item in
                  HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(item.path)
                      .font(.caption.monospaced())
                      .lineLimit(1)
                      .truncationMode(.middle)
                    Spacer(minLength: 8)
                    Text(item.reportedSize ?? "Size not reported")
                      .font(.caption.weight(.medium))
                      .foregroundStyle(.secondary)
                  }
                }
              }
              .padding(.vertical, 8)
            } label: {
              HStack {
                Text(category.name).font(.headline)
                Spacer()
                Text(NirmokaFormat.items(category.items.reduce(UInt64(0)) { $0 + $1.itemCount }))
                  .font(.caption)
                  .foregroundStyle(.secondary)
              }
            }
            .padding(14)
            .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 12))
          }
        }
      }

      HStack {
        Text("Previewed \(preview.generatedAt.formattedDate)")
          .font(.caption)
          .foregroundStyle(.secondary)
        Spacer()
        Button("Run Mole Cleanup", systemImage: "sparkles") { model.prepareCleanup() }
          .buttonStyle(.borderedProminent)
          .disabled(preview.totalItems == 0 || model.isExecutingCleanup)
      }
    }
  }

  private func operationCard(_ operation: CleanupOperation) -> some View {
    VStack(alignment: .leading, spacing: 12) {
      Label(
        cleanupCompletionText(operation.completion),
        systemImage: operation.completion == .finished
          ? "checkmark.circle.fill" : "exclamationmark.triangle.fill"
      )
      .font(.title3.weight(.semibold))
      .foregroundStyle(operation.completion == .finished ? NirmokaPalette.accent : .orange)
      Text(
        "Mole reviewed \(NirmokaFormat.items(operation.reviewedItems)) with a potential cleanup of \(operation.reviewedPotentialCleanup ?? "an unreported amount")."
      )
      .foregroundStyle(.secondary)
      if !operation.warnings.isEmpty { warningsCard(operation.warnings) }
      Button("Get a Fresh Preview", systemImage: "arrow.clockwise") { model.loadCleanupPreview() }
        .buttonStyle(.bordered)
    }
    .padding(20)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 16))
  }

  private func warningsCard(_ warnings: [String]) -> some View {
    VStack(alignment: .leading, spacing: 7) {
      Label("Mole reported", systemImage: "exclamationmark.triangle")
        .font(.subheadline.weight(.semibold))
      ForEach(Array(warnings.enumerated()), id: \.offset) { _, warning in
        Text(warning).font(.caption).foregroundStyle(.secondary)
      }
    }
    .padding(14)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(Color.orange.opacity(0.08), in: RoundedRectangle(cornerRadius: 12))
  }

  private func progressSymbol(_ kind: CleanupProgress.Kind) -> String {
    switch kind {
    case .category: "folder"
    case .item: "doc"
    case .categoryTotal: "sum"
    }
  }
}

struct ApplicationsView: View {
  @EnvironmentObject private var model: AppModel
  let showInspector: () -> Void
  private var search: String { model.applicationSearch }
  @State private var reviewPresented = false

  private var filteredApplications: [InstalledApplication] {
    (model.applicationInventory?.rows ?? []).filter {
      search.isEmpty || $0.name.localizedCaseInsensitiveContains(search)
        || $0.bundleId.localizedCaseInsensitiveContains(search)
    }
  }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 18) {
        HStack(alignment: .top) {
          VStack(alignment: .leading, spacing: 6) {
            Text("SOFTWARE")
              .font(.caption.weight(.bold))
              .tracking(1.2)
              .foregroundStyle(NirmokaPalette.accentDeep)
            Text("Applications")
              .font(.system(size: 28, weight: .semibold, design: .rounded))
            Text("Inspect an app’s storage, or select applications to review their removal.")
              .font(.callout)
              .foregroundStyle(.secondary)
          }
          Spacer()
          Button("Refresh", systemImage: "arrow.clockwise") { model.loadApplications(force: true) }
            .buttonStyle(.bordered)
        }

        if !model.canListApplications && !model.isCheckingBackends {
          BackendSetupView(scanning: false)
        } else if model.isLoadingApplications {
          ProgressView("Loading installed applications…")
            .padding(20)
        } else if let inventory = model.applicationInventory {
          Text("\(inventory.total) applications · reported by \(inventory.backend)")
            .font(.caption)
            .foregroundStyle(.secondary)

          TextField("Search applications", text: $model.applicationSearch)
            .textFieldStyle(.roundedBorder)
          if filteredApplications.isEmpty {
            NirmokaEmptyState(
              title: "No matching applications", symbol: "magnifyingglass",
              description: "Try another name or clear the search.")
          }
          LazyVStack(spacing: 4) {
            ForEach(filteredApplications) { application in
              let selected = model.selectedApplications.contains(application.uninstallName)
              HStack(spacing: 12) {
                Toggle(
                  "Select \(application.name) for removal",
                  isOn: Binding(
                    get: { selected }, set: { _ in model.toggleApplication(application) })
                )
                .labelsHidden().toggleStyle(.checkbox)
                .disabled(
                  !model.canUninstallApplications || model.isExecutingUninstall
                    || model.isLoadingUninstallPreview)
                Button {
                  model.inspectInstalledApplication(application)
                  showInspector()
                } label: {
                  HStack(spacing: 12) {
                    Image(systemName: "app.fill")
                      .font(.title3)
                      .foregroundStyle(Color.secondary)
                    VStack(alignment: .leading, spacing: 3) {
                      Text(application.name).font(.body.weight(.medium))
                      Text("\(application.source) · \(application.bundleId)")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Text(application.reportedSize)
                      .font(.callout.monospacedDigit())
                      .foregroundStyle(.secondary)
                  }
                  .padding(13)
                  .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("Inspect \(application.name)’s storage")
              }
              .background(
                selected ? Color.accentColor.opacity(0.1) : NirmokaPalette.card,
                in: RoundedRectangle(cornerRadius: 11))
            }
          }
        }

        if let result = model.uninstallResult {
          uninstallResultCard(result)
        }
      }
      .padding(28)
      .frame(maxWidth: 1100, alignment: .leading)
      .frame(maxWidth: .infinity)
    }
    .background(NirmokaPalette.canvas)
    .onAppear {
      if model.canListApplications { model.loadApplications() }
      if model.uninstallPreview != nil || model.isLoadingUninstallPreview { reviewPresented = true }
    }
    .onChange(of: model.canListApplications) { available in
      if available { model.loadApplications() }
    }
    .onChange(of: model.isLoadingUninstallPreview) { loading in
      if loading { reviewPresented = true }
    }
    .sheet(isPresented: $reviewPresented) {
      VStack(alignment: .leading, spacing: 16) {
        HStack {
          Text("Review application removal").font(.title2.bold())
          Spacer()
          Button("Done") { reviewPresented = false }.disabled(model.isExecutingUninstall)
        }
        if model.isExecutingUninstall {
          ProgressView("Removing the reviewed applications…")
          Text("Stopping cannot restore anything already removed.").font(.caption).foregroundStyle(
            .secondary)
          Button("Stop") { model.cancelUninstall() }
        } else if model.isLoadingUninstallPreview {
          ProgressView("Preparing removal review…")
          Button("Cancel") {
            model.cancelUninstallPreview()
            reviewPresented = false
          }
        } else if let preview = model.uninstallPreview {
          ScrollView { uninstallReview(preview) }
        } else if let result = model.uninstallResult {
          ScrollView { uninstallResultCard(result) }
        } else {
          Text(
            model.errorMessage ?? "No removal plan is available. Close this review and try again."
          ).foregroundStyle(.secondary)
        }
      }
      .padding(24)
      .frame(minWidth: 650, idealWidth: 740, minHeight: 400, idealHeight: 600)
      .interactiveDismissDisabled(model.isExecutingUninstall)
      .confirmationDialog(
        "Uninstall selected applications?",
        isPresented: $model.isUninstallConfirmationPresented,
        titleVisibility: .visible
      ) {
        Button("Move to Trash", role: .destructive) { model.confirmUninstall() }
        Button("Cancel", role: .cancel) {}
      } message: {
        if let preparation = model.uninstallPreparation {
          Text(
            "Mole will remove \(preparation.applications.joined(separator: ", ")) and route removable items to the Trash. Review-only and protected items stay untouched."
          )
        }
      }

    }
    .safeAreaInset(edge: .bottom) {
      if !model.selectedApplications.isEmpty {
        HStack {
          Text("\(model.selectedApplications.count) selected")
            .font(.callout.weight(.medium))
          Spacer()
          if model.isLoadingUninstallPreview {
            ProgressView("Building Mole’s removal plan…")
            Button("Cancel") { model.cancelUninstallPreview() }
              .buttonStyle(.bordered)
          } else {
            Button("Review Removal", systemImage: "doc.text.magnifyingglass") {
              reviewPresented = true
              model.previewUninstall()
            }
            .disabled(model.isExecutingUninstall || !model.canUninstallApplications)
            .buttonStyle(.borderedProminent)
          }
        }
        .padding(.horizontal, 28)
        .padding(.vertical, 14)
        .background(.bar)
      }
    }
  }

  private func uninstallReview(_ preview: UninstallPreview) -> some View {
    VStack(alignment: .leading, spacing: 12) {
      SectionHeading(
        title: "Mole’s removal review",
        subtitle:
          "No files are removed until you confirm. The backend owns these paths and the safety checks."
      )
      HStack(spacing: 12) {
        MetricCard(
          title: "Potential space", value: preview.reportedTotal ?? "Not reported",
          symbol: "externaldrive")
        MetricCard(
          title: "Items in plan", value: NirmokaFormat.count(preview.totalItems), symbol: "doc.text"
        )
        MetricCard(title: "Mole", value: preview.backendVersion, symbol: "terminal")
      }
      ForEach(preview.apps) { app in
        DisclosureGroup {
          VStack(alignment: .leading, spacing: 8) {
            ForEach(app.items) { item in
              HStack(alignment: .top, spacing: 8) {
                Image(systemName: uninstallItemSymbol(item.scope))
                  .foregroundStyle(uninstallItemColor(item.scope))
                VStack(alignment: .leading, spacing: 3) {
                  Text(item.displayPath)
                    .font(.caption.monospaced())
                    .textSelection(.enabled)
                  Text(uninstallItemLabel(item.scope))
                    .font(.caption2)
                    .foregroundStyle(.secondary)
                }
                Spacer()
                Text(item.reportedSize ?? "")
                  .font(.caption)
                  .foregroundStyle(.secondary)
              }
            }
          }
          .padding(.vertical, 8)
        } label: {
          HStack {
            Text(app.name).font(.headline)
            if app.homebrewCask {
              Text("Homebrew").font(.caption2).foregroundStyle(.secondary)
            }
            Spacer()
            Text(app.reportedSize ?? "Size not reported")
              .font(.caption.weight(.medium))
              .foregroundStyle(.secondary)
          }
        }
        .padding(14)
        .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 11))
      }
      if !preview.notes.isEmpty { warningsCard(preview.notes, title: "Mole says will remain") }
      if !preview.warnings.isEmpty { warningsCard(preview.warnings, title: "Mole reported") }
      DisclosureGroup("Show Mole’s full transcript") {
        Text(preview.transcript)
          .font(.caption.monospaced())
          .textSelection(.enabled)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(.vertical, 8)
      }
      .padding(14)
      .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 11))
      HStack {
        Spacer()
        Button("Uninstall…", systemImage: "trash") { model.prepareUninstall() }
          .buttonStyle(.borderedProminent)
          .tint(.red)
          .disabled(
            preview.totalItems == 0 || model.isExecutingUninstall
              || Set(preview.requested) != model.selectedApplications)
      }
    }
  }

  private func warningsCard(_ values: [String], title: String) -> some View {
    VStack(alignment: .leading, spacing: 6) {
      Text(title).font(.subheadline.weight(.semibold))
      ForEach(Array(values.enumerated()), id: \.offset) { _, value in
        Text(value).font(.caption).foregroundStyle(.secondary)
      }
    }
    .padding(14)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(Color.orange.opacity(0.08), in: RoundedRectangle(cornerRadius: 11))
  }

  private func uninstallResultCard(_ result: UninstallOperation) -> some View {
    VStack(alignment: .leading, spacing: 8) {
      Label(
        uninstallCompletionText(result.completion),
        systemImage: result.completion == .finished
          ? "checkmark.circle.fill" : "exclamationmark.triangle.fill"
      )
      .font(.headline)
      Text(
        "Mole reported \(result.removed.count) removed and \(result.failed.count) failed. Reported space: \(result.reportedFreed ?? "not provided")"
      )
      .font(.callout)
      .foregroundStyle(.secondary)
      if !result.warnings.isEmpty { warningsCard(result.warnings, title: "Mole reported") }
    }
    .padding(16)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 12))
  }
}

struct SystemStatusView: View {
  @EnvironmentObject private var model: AppModel

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 18) {
        HStack {
          VStack(alignment: .leading, spacing: 6) {
            Text("SYSTEM STATUS")
              .font(.caption.weight(.bold))
              .tracking(1.2)
              .foregroundStyle(NirmokaPalette.accentDeep)
            Text("How your Mac is doing")
              .font(.system(size: 28, weight: .semibold, design: .rounded))
            Text(
              "A snapshot from Mole. Temperatures and some health readings depend on the hardware."
            )
            .font(.callout)
            .foregroundStyle(.secondary)
          }
          Spacer()
          Button("Refresh", systemImage: "arrow.clockwise") { model.loadSystemStatus(force: true) }
            .buttonStyle(.bordered)
        }

        if model.isLoadingSystemStatus {
          ProgressView("Reading system status…").padding(20)
        } else if let status = model.systemStatus {
          HStack(spacing: 12) {
            MetricCard(
              title: "Health score", value: "\(status.healthScore)", symbol: "heart.text.square")
            MetricCard(
              title: "CPU",
              value: status.cpu.usage.formatted(.number.precision(.fractionLength(0))) + "%",
              symbol: "cpu")
            MetricCard(
              title: "Memory pressure", value: status.memory.pressure.capitalized,
              symbol: "memorychip")
          }
          Text(status.healthScoreMessage)
            .font(.callout)
            .foregroundStyle(.secondary)
          LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 12) {
            statusCard(
              "Hardware", symbol: "desktopcomputer",
              rows: [
                ("Model", status.hardware.model),
                ("Processor", status.hardware.cpuModel),
                ("Memory", status.hardware.totalRam),
                ("macOS", status.hardware.osVersion),
                ("Uptime", status.uptime),
              ])
            statusCard(
              "CPU & Memory", symbol: "cpu",
              rows: [
                (
                  "Load average",
                  "\(status.cpu.load1.formatted(.number.precision(.fractionLength(2)))) / \(status.cpu.load5.formatted(.number.precision(.fractionLength(2)))) / \(status.cpu.load15.formatted(.number.precision(.fractionLength(2))))"
                ),
                ("Logical CPUs", "\(status.cpu.logicalCpu)"),
                (
                  "Memory used",
                  "\(NirmokaFormat.bytes(status.memory.used)) of \(NirmokaFormat.bytes(status.memory.total))"
                ),
                ("Available", NirmokaFormat.bytes(status.memory.available)),
                (
                  "Swap",
                  "\(NirmokaFormat.bytes(status.memory.swapUsed)) of \(NirmokaFormat.bytes(status.memory.swapTotal))"
                ),
              ])
            statusCard(
              "Disks", symbol: "externaldrive",
              rows: status.disks.map {
                (
                  $0.mount,
                  "\(NirmokaFormat.bytes($0.used)) of \(NirmokaFormat.bytes($0.total)) · \($0.smartStatus)"
                )
              })
            statusCard(
              "Thermal", symbol: "thermometer.medium",
              rows: [
                ("CPU", temperature(status.thermal.cpuTemp)),
                ("GPU", temperature(status.thermal.gpuTemp)),
                ("Battery", temperature(status.thermal.batteryTemp)),
                (
                  "Fans",
                  status.thermal.fanSpeed.map {
                    "\($0.formatted(.number.precision(.fractionLength(0)))) RPM"
                  } ?? "Not reported"
                ),
              ])
            if !status.batteries.isEmpty {
              statusCard(
                "Battery", symbol: "battery.100",
                rows: status.batteries.map {
                  (
                    "\($0.status.capitalized) · \($0.percent.formatted(.number.precision(.fractionLength(0))))%",
                    "\($0.health) · \($0.cycleCount) cycles · \($0.timeLeft)"
                  )
                })
            }
          }
          Text(
            "Updated \(status.collectedAt.formattedDate) · Mole on \(status.platform) · \(status.host)"
          )
          .font(.caption)
          .foregroundStyle(.tertiary)
        } else {
          NirmokaEmptyState(
            title: "No status yet", symbol: "waveform.path.ecg",
            description: "Load a fresh snapshot from Mole to see available health metrics."
          )
          .frame(maxWidth: .infinity)
          .padding(20)
          .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 14))
        }
      }
      .padding(28)
      .frame(maxWidth: 1100, alignment: .leading)
      .frame(maxWidth: .infinity)
    }
    .background(NirmokaPalette.canvas)
    .onAppear { model.loadSystemStatus() }
  }

  private func statusCard(_ title: String, symbol: String, rows: [(String, String)]) -> some View {
    VStack(alignment: .leading, spacing: 12) {
      Label(title, systemImage: symbol).font(.headline)
      ForEach(Array(rows.enumerated()), id: \.offset) { _, row in
        HStack(alignment: .firstTextBaseline, spacing: 10) {
          Text(row.0).foregroundStyle(.secondary)
          Spacer(minLength: 8)
          Text(row.1).multilineTextAlignment(.trailing)
        }
        .font(.caption)
      }
    }
    .padding(16)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 13))
  }

  private func temperature(_ value: Double?) -> String {
    value.map { "\($0.formatted(.number.precision(.fractionLength(1)))) °C" } ?? "Not reported"
  }
}

struct ActivityView: View {
  @EnvironmentObject private var model: AppModel

  private var entries: [ActivityEntry] {
    (model.trashOperations.map(ActivityEntry.trash)
      + model.deleteOperations.map(ActivityEntry.deletion)
      + model.cleanupOperations.map(ActivityEntry.cleanup)
      + model.uninstallOperations.map(ActivityEntry.uninstall))
      .sorted { $0.timestamp > $1.timestamp }
  }

  private var isEmpty: Bool {
    entries.isEmpty
  }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 18) {
        HStack {
          VStack(alignment: .leading, spacing: 6) {
            Text("ACTIVITY")
              .font(.caption.weight(.bold))
              .tracking(1.2)
              .foregroundStyle(NirmokaPalette.accentDeep)
            Text("Your cleanup history")
              .font(.system(size: 28, weight: .semibold, design: .rounded))
            Text("See items moved to Trash, cleanup runs, and app removals in one place.")
              .font(.callout)
              .foregroundStyle(.secondary)
          }
          Spacer(minLength: 12)
          Button("Refresh", systemImage: "arrow.clockwise") { Task { await model.loadActivity() } }
            .buttonStyle(.bordered)
            .disabled(model.isLoadingActivity)
        }

        if model.isLoadingActivity && isEmpty {
          ProgressView("Loading activity…")
            .frame(maxWidth: .infinity)
            .padding(20)
            .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 14))
        } else if isEmpty {
          NirmokaEmptyState(
            title: "No activity yet", symbol: "clock.arrow.circlepath",
            description: "Items you move to Trash, cleanup runs, and app removals will appear here."
          )
          .frame(maxWidth: .infinity)
          .padding(20)
          .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 14))
        } else {
          LazyVStack(spacing: 10) {
            ForEach(entries) { entry in
              activityCard(entry)
            }
          }
        }
      }
      .padding(28)
      .frame(maxWidth: 1100, alignment: .leading)
      .frame(maxWidth: .infinity)
    }
    .background(NirmokaPalette.canvas)
    .task { await model.loadActivity() }
  }

  @ViewBuilder
  private func activityCard(_ entry: ActivityEntry) -> some View {
    VStack(alignment: .leading, spacing: 7) {
      HStack {
        Label(entry.title, systemImage: entry.symbol)
          .font(.headline)
        Spacer()
        Text(entry.timestamp.formattedDate)
          .font(.caption)
          .foregroundStyle(.secondary)
      }
      Text(entry.detail)
        .font(.callout)
        .foregroundStyle(.secondary)
      if let note = entry.note {
        Text(note)
          .font(.caption)
          .foregroundStyle(entry.isWarning ? .orange : .secondary)
      }
    }
    .padding(14)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 11))
  }
}

private enum ActivityEntry: Identifiable {
  case trash(TrashOperation)
  case deletion(DeleteOperation)
  case cleanup(CleanupOperation)
  case uninstall(UninstallOperation)

  var id: String {
    switch self {
    case .trash(let value): "trash-\(value.id)"
    case .deletion(let value): "deletion-\(value.id)"
    case .cleanup(let value): "cleanup-\(value.id)"
    case .uninstall(let value): "uninstall-\(value.id)"
    }
  }

  var timestamp: UInt64 {
    switch self {
    case .trash(let value): value.trashedAtMs
    case .deletion(let value): value.undoneAtMs ?? value.deletedAtMs
    case .cleanup(let value): value.executedAtMs
    case .uninstall(let value): value.executedAtMs
    }
  }

  var title: String {
    switch self {
    case .trash: "Moved to Trash"
    case .deletion(let value): value.undone ? "Restored item" : "Recoverable removal"
    case .cleanup(let value): cleanupCompletionText(value.completion)
    case .uninstall(let value): uninstallCompletionText(value.completion)
    }
  }

  var symbol: String {
    switch self {
    case .trash, .deletion: "trash"
    case .cleanup: "sparkles"
    case .uninstall: "app.dashed"
    }
  }

  var detail: String {
    switch self {
    case .trash(let value):
      "\(NirmokaFormat.leaf(value.targetPath)) · \(NirmokaFormat.bytes(value.totalBytes)) · Restore with Put Back in Finder"
    case .deletion(let value):
      "\(NirmokaFormat.leaf(value.targetPath)) · \(value.undone ? "Restored" : "Recovery receipt recorded") · \(value.backend)"
    case .cleanup(let value):
      "Reviewed \(NirmokaFormat.items(value.reviewedItems)) · potential \(value.reviewedPotentialCleanup ?? "not reported") · Mole \(value.backendVersion)"
    case .uninstall(let value):
      "\(value.reviewedApplications.joined(separator: ", ")) · Mole reported \(value.removed.count) removed · \(value.failed.count) failed · freed \(value.reportedFreed ?? "not reported")"
    }
  }

  var note: String? {
    switch self {
    case .trash(let value): value.logError.map { "History note: \($0)" }
    case .deletion(let value): value.logError.map { "History note: \($0)" }
    case .cleanup(let value):
      value.logError ?? value.warnings.first
    case .uninstall(let value):
      value.logError ?? value.warnings.first
    }
  }

  var isWarning: Bool {
    switch self {
    case .cleanup(let value):
      value.completion != .finished || value.logError != nil || !value.warnings.isEmpty
    case .uninstall(let value):
      value.completion != .finished || value.logError != nil || !value.warnings.isEmpty
    case .trash(let value): value.logError != nil
    case .deletion(let value): value.logError != nil
    }
  }
}

private struct MetricCard: View {
  let title: String
  let value: String
  let symbol: String

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      Label(title, systemImage: symbol)
        .font(.caption.weight(.medium))
        .foregroundStyle(.secondary)
      Text(value)
        .font(.title3.weight(.semibold))
        .lineLimit(1)
        .minimumScaleFactor(0.75)
    }
    .padding(15)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(NirmokaPalette.card, in: RoundedRectangle(cornerRadius: 12))
  }
}

private func cleanupScopeText(_ scope: CleanupSystemScope) -> String {
  switch scope {
  case .included: "Mole’s preview includes system-level candidates."
  case .userOnly: "Mole’s preview covers user-level candidates."
  case .unknown: "Mole did not report whether system-level candidates are included."
  }
}

private func cleanupCompletionText(_ completion: CleanupCompletion) -> String {
  switch completion {
  case .finished: "Cleanup finished"
  case .partial: "Cleanup partially finished"
  case .cancelled: "Cleanup stopped"
  case .failed: "Cleanup failed"
  }
}

private func uninstallCompletionText(_ completion: UninstallCompletion) -> String {
  switch completion {
  case .finished: "Removal finished"
  case .partial: "Removal partially finished"
  case .cancelled: "Removal stopped"
  case .failed: "Removal failed"
  }
}

private func uninstallItemSymbol(_ scope: UninstallItem.Scope) -> String {
  switch scope {
  case .removed: "trash"
  case .system: "lock"
  case .reviewOnly: "eye"
  }
}

private func uninstallItemColor(_ scope: UninstallItem.Scope) -> Color {
  switch scope {
  case .removed: NirmokaPalette.accentDeep
  case .system, .reviewOnly: .secondary
  }
}

private func uninstallItemLabel(_ scope: UninstallItem.Scope) -> String {
  switch scope {
  case .removed: "Mole plans to remove this item."
  case .system: "Mole says this system item will remain."
  case .reviewOnly: "Review-only item; Mole will not remove it."
  }
}

extension String {
  fileprivate var formattedDate: String {
    self.replacingOccurrences(of: "T", with: " ").replacingOccurrences(of: "Z", with: " UTC")
  }
}

extension UInt64 {
  fileprivate var formattedDate: String {
    Date(timeIntervalSince1970: TimeInterval(self) / 1_000)
      .formatted(date: .abbreviated, time: .shortened)
  }
}
