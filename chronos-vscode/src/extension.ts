import * as vscode from 'vscode';
import { spawn, ChildProcess } from 'child_process';
import * as readline from 'readline';
import * as path from 'path';
import * as fs from 'fs';
import * as https from 'https';

let mcpProcess: ChildProcess | null = null;
let requestId = 1;
const pendingRequests = new Map<number, (result: any) => void>();
const outputChannel = vscode.window.createOutputChannel("CHRONOS Agent");

export async function activate(context: vscode.ExtensionContext) {
    const config = vscode.workspace.getConfiguration('chronos');
    let binaryPath = config.get<string>('mcpBinaryPath');
    
    if (!binaryPath || binaryPath.trim() === '') {
        const ext = process.platform === 'win32' ? '.exe' : '';
        const bundledPath = path.join(context.globalStorageUri.fsPath, `chronos-mcp${ext}`);
        
        if (fs.existsSync(bundledPath)) {
            binaryPath = bundledPath;
        } else {
            // Ask user to download or configure
            const action = await vscode.window.showWarningMessage(
                "CHRONOS Agent MCP binary not found. Do you want to download the pre-compiled binary?",
                "Download", "Configure Path"
            );
            
            if (action === "Download") {
                binaryPath = await downloadBinary(context);
                if (!binaryPath) return;
            } else if (action === "Configure Path") {
                vscode.commands.executeCommand('workbench.action.openSettings', 'chronos.mcpBinaryPath');
                return;
            } else {
                return;
            }
        }
    }
    
    // Start MCP server
    startMcpServer(binaryPath!);

    // Analyze on file save
    vscode.workspace.onDidSaveTextDocument(async (doc) => {
        if (doc.languageId === 'typescript' || doc.languageId === 'javascript') {
            await analyzeFile(doc.fileName);
        }
    });

    // Commands
    context.subscriptions.push(
        vscode.commands.registerCommand('chronos.analyze', async () => {
            const editor = vscode.window.activeTextEditor;
            if (editor) await analyzeFile(editor.document.fileName);
        }),
        vscode.commands.registerCommand('chronos.showHotspots', showHotspots),
        vscode.commands.registerCommand('chronos.blastRadius', async () => {
            const symbol = await vscode.window.showInputBox({ prompt: 'Symbol name?' });
            if (symbol) await showBlastRadius(symbol);
        })
    );
}

function startMcpServer(binaryPath: string) {
    const workspace = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath || '.';
    outputChannel.appendLine(`Starting MCP Server from: ${binaryPath}`);
    
    try {
        mcpProcess = spawn(binaryPath, ['--workspace', workspace]);
        
        const rl = readline.createInterface({ input: mcpProcess.stdout! });
        rl.on('line', (line) => {
            try {
                const msg = JSON.parse(line);
                if (msg.id && pendingRequests.has(msg.id)) {
                    pendingRequests.get(msg.id)!(msg.result);
                    pendingRequests.delete(msg.id);
                }
            } catch {}
        });

        mcpProcess.stderr?.on('data', (data) => {
            outputChannel.appendLine(`[MCP STDERR] ${data.toString()}`);
        });

        mcpProcess.on('exit', (code) => {
            outputChannel.appendLine(`MCP Server exited with code ${code}`);
        });
        
        // Initialize
        sendMcpRequest('initialize', { rootPath: workspace });
        outputChannel.appendLine("MCP Server started successfully.");
    } catch (e) {
        vscode.window.showErrorMessage(`Failed to start MCP server: ${e}`);
    }
}

function sendMcpRequest(method: string, params: any): Promise<any> {
    return new Promise((resolve) => {
        const id = requestId++;
        pendingRequests.set(id, resolve);
        const msg = JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n';
        mcpProcess?.stdin?.write(msg);
    });
}

async function analyzeFile(filePath: string) {
    if (!mcpProcess) return;
    vscode.window.withProgress({
        location: vscode.ProgressLocation.Window,
        title: "CHRONOS: Analyzing...",
    }, async () => {
        await sendMcpRequest('tools/call', {
            name: 'chronos_analyze_file',
            arguments: { file_path: filePath }
        });
        vscode.window.setStatusBarMessage('$(check) CHRONOS: Analyzed', 3000);
    });
}

async function showHotspots() {
    if (!mcpProcess) return;
    const result = await sendMcpRequest('tools/call', {
        name: 'chronos_query_hotspots',
        arguments: { top_n: 10 }
    });
    
    const panel = vscode.window.createWebviewPanel(
        'chronosHotspots', 'CHRONOS: Hotspots', vscode.ViewColumn.Beside, {}
    );
    panel.webview.html = `<pre style="padding: 20px;">${result?.content?.[0]?.text || 'No data'}</pre>`;
}

async function showBlastRadius(symbol: string) {
    if (!mcpProcess) return;
    const result = await sendMcpRequest('tools/call', {
        name: 'chronos_blast_radius',
        arguments: { symbol_name: symbol }
    });
    
    const panel = vscode.window.createWebviewPanel(
        'chronosBlastRadius', `CHRONOS: ${symbol}`, vscode.ViewColumn.Beside,
        { enableScripts: true }
    );
    // Try to render markdown gracefully if it's text
    panel.webview.html = `<div style="padding: 20px; font-family: sans-serif;">
        <h2>Blast Radius: ${symbol}</h2>
        <pre>${result?.content?.[0]?.text || 'Analysis failed'}</pre>
    </div>`;
}

async function downloadBinary(context: vscode.ExtensionContext): Promise<string | undefined> {
    const ext = process.platform === 'win32' ? '.exe' : '';
    const destDir = context.globalStorageUri.fsPath;
    if (!fs.existsSync(destDir)) {
        fs.mkdirSync(destDir, { recursive: true });
    }
    
    const destFile = path.join(destDir, `chronos-mcp${ext}`);
    
    return await vscode.window.withProgress({
        location: vscode.ProgressLocation.Notification,
        title: "Downloading CHRONOS MCP binary...",
        cancellable: false
    }, async (progress) => {
        // In a real scenario, this would hit GitHub API to find latest release assets.
        // For this demo, we'll guide them to the actual built binary in target/release if it exists,
        // or just mock it since we can't easily download from a local repo.
        
        vscode.window.showInformationMessage("In production, this downloads from GitHub Releases via CI/CD. Please point to the local target/release binary for now.");
        vscode.commands.executeCommand('workbench.action.openSettings', 'chronos.mcpBinaryPath');
        return undefined;
    });
}

export function deactivate() {
    mcpProcess?.kill();
}
