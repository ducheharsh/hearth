// Hearth.app: a small menu bar companion for the hearth daemon.
//
// The daemon (Contents/Resources/hearthd) runs as its own launchd login service, so
// your apps keep running whether or not this menu is open. The menu shows status,
// opens the dashboard, copies the server address and toggles start-at-login.

import AppKit
import ServiceManagement

private let api = URL(string: "http://127.0.0.1:9000/api")!
private let dashboard = URL(string: "http://127.0.0.1:9000")!

struct AccessLink: Decodable { let kind: String; let url: String }
struct Access: Decodable { let links: [AccessLink] }
struct Health: Decodable { let ok: Bool; let version: String }

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    private let menu = NSMenu()
    private let statusLine = NSMenuItem(title: "Checking…", action: nil, keyEquivalent: "")
    private let addressLine = NSMenuItem(title: "", action: #selector(copyAddress), keyEquivalent: "c")
    private let loginItem = NSMenuItem(title: "Start hearth at Login", action: #selector(toggleService), keyEquivalent: "")
    private var running = false
    private var address: String?
    private var timer: Timer?

    private var daemon: String { Bundle.main.path(forResource: "hearthd", ofType: nil) ?? "/opt/homebrew/bin/hearthd" }
    private var plist: URL {
        FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/LaunchAgents/dev.hearth.hearthd.plist")
    }
    private var brewService: Bool {
        FileManager.default.fileExists(atPath: FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent("Library/LaunchAgents/homebrew.mxcl.hearth.plist").path)
    }

    func applicationDidFinishLaunching(_: Notification) {
        if let icon = Bundle.main.image(forResource: "MenuIcon") {
            icon.size = NSSize(width: 19.5, height: 18)
            item.button?.image = icon
        }
        item.button?.toolTip = "hearth"
        statusLine.isEnabled = false
        addressLine.target = self
        loginItem.target = self
        menu.addItem(statusLine)
        menu.addItem(addressLine)
        menu.addItem(.separator())
        menu.addItem(withTitle: "Open Dashboard", action: #selector(openDashboard), keyEquivalent: "o").target = self
        menu.addItem(withTitle: "Restart hearth", action: #selector(restart), keyEquivalent: "r").target = self
        menu.addItem(withTitle: "Show Logs", action: #selector(showLogs), keyEquivalent: "l").target = self
        menu.addItem(.separator())
        menu.addItem(loginItem)
        let menuAtLogin = NSMenuItem(title: "Show This Menu at Login", action: #selector(toggleMenuAtLogin), keyEquivalent: "")
        menuAtLogin.target = self
        menu.addItem(menuAtLogin)
        menu.addItem(.separator())
        menu.addItem(withTitle: "Quit Menu (hearth keeps running)", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        item.menu = menu

        if moveToApplicationsIfNeeded() { return }
        // First launch: start the daemon as a login service, unless a hearth (Homebrew,
        // a dev build, an earlier install) is already answering.
        get("/health") { (h: Health?) in
            if h == nil && !FileManager.default.fileExists(atPath: self.plist.path) && !self.brewService {
                self.run(self.daemon, ["service", "install"])
            }
            self.refresh()
        }
        timer = Timer.scheduledTimer(withTimeInterval: 5, repeats: true) { [weak self] _ in self?.refresh() }
    }

    /// The login service points at the binary inside this app, so the app must live in
    /// Applications (not a mounted DMG or Downloads). Returns true if we relaunched.
    private func moveToApplicationsIfNeeded() -> Bool {
        let here = Bundle.main.bundleURL
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        if here.path.hasPrefix("/Applications/") || here.path.hasPrefix(home + "/Applications/") { return false }
        let alert = NSAlert()
        alert.messageText = "Move Hearth to Applications?"
        alert.informativeText = "hearth runs in the background from here, so it needs to live in your Applications folder."
        alert.addButton(withTitle: "Move to Applications")
        alert.addButton(withTitle: "Quit")
        NSApp.activate(ignoringOtherApps: true)
        guard alert.runModal() == .alertFirstButtonReturn else { NSApp.terminate(nil); return true }
        let dest = URL(fileURLWithPath: "/Applications/Hearth.app")
        do {
            if FileManager.default.fileExists(atPath: dest.path) { try FileManager.default.removeItem(at: dest) }
            try FileManager.default.copyItem(at: here, to: dest)
        } catch {
            let e = NSAlert(error: error)
            e.runModal()
            NSApp.terminate(nil)
            return true
        }
        NSWorkspace.shared.openApplication(at: dest, configuration: NSWorkspace.OpenConfiguration()) { _, _ in
            DispatchQueue.main.async { NSApp.terminate(nil) }
        }
        return true
    }

    private func refresh() {
        loginItem.state = (FileManager.default.fileExists(atPath: plist.path) || brewService) ? .on : .off
        if #available(macOS 13.0, *) {
            menu.items.first { $0.action == #selector(toggleMenuAtLogin) }?.state = SMAppService.mainApp.status == .enabled ? .on : .off
        }
        get("/health") { (h: Health?) in
            self.running = h?.ok == true
            self.statusLine.title = self.running ? "● hearth \(h?.version ?? "") is running" : "○ hearth is not running"
            self.item.button?.appearsDisabled = !self.running
        }
        get("/access") { (a: Access?) in
            let best = a?.links.first { $0.kind != "local" } ?? a?.links.first
            self.address = best?.url
            self.addressLine.title = best.map { "Copy \($0.url.replacingOccurrences(of: "https://", with: "").replacingOccurrences(of: "http://", with: ""))" } ?? ""
            self.addressLine.isHidden = best == nil
        }
    }

    private func get<T: Decodable>(_ path: String, _ done: @escaping (T?) -> Void) {
        var req = URLRequest(url: api.appendingPathComponent(path))
        req.timeoutInterval = 3
        URLSession.shared.dataTask(with: req) { data, _, _ in
            let value = data.flatMap { try? JSONDecoder().decode(T.self, from: $0) }
            DispatchQueue.main.async { done(value) }
        }.resume()
    }

    @discardableResult
    private func run(_ exe: String, _ args: [String]) -> Int32 {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: exe)
        p.arguments = args
        do { try p.run(); p.waitUntilExit(); return p.terminationStatus } catch { return -1 }
    }

    @objc private func openDashboard() { NSWorkspace.shared.open(dashboard) }

    @objc private func copyAddress() {
        guard let a = address else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(a, forType: .string)
    }

    @objc private func restart() {
        if brewService {
            run("/bin/sh", ["-lc", "brew services restart hearth"])
        } else {
            run("/bin/launchctl", ["kickstart", "-k", "gui/\(getuid())/dev.hearth.hearthd"])
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) { self.refresh() }
    }

    @objc private func showLogs() {
        let log = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(".hearth/logs/hearthd.log")
        NSWorkspace.shared.open(log)
    }

    @objc private func toggleService() {
        if brewService {
            let a = NSAlert()
            a.messageText = "hearth is managed by Homebrew"
            a.informativeText = "Use `brew services stop hearth` or `brew services start hearth` in Terminal."
            a.runModal()
            return
        }
        let installed = FileManager.default.fileExists(atPath: plist.path)
        run(daemon, ["service", installed ? "uninstall" : "install"])
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) { self.refresh() }
    }

    @objc private func toggleMenuAtLogin() {
        guard #available(macOS 13.0, *) else { return }
        let s = SMAppService.mainApp
        do { if s.status == .enabled { try s.unregister() } else { try s.register() } } catch { NSSound.beep() }
        refresh()
    }
}

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.setActivationPolicy(.accessory)
app.run()
