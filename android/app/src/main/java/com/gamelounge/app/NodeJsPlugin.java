package com.gamelounge.app;

import android.content.Context;
import android.util.Log;

import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;

import java.io.*;
import java.io.BufferedReader;
import java.io.FileReader;
import java.lang.reflect.Field;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.util.concurrent.atomic.AtomicBoolean;

@CapacitorPlugin(name = "NodeJs")
public class NodeJsPlugin extends Plugin {

    private static final String TAG = "NodeJsPlugin";
    private Process nodeProcess;
    private final AtomicBoolean serverReady = new AtomicBoolean(false);
    private static final int SERVER_PORT = 3001;

    @PluginMethod
    public void start(PluginCall call) {
        startServer();
        new Thread(() -> {
            try {
                for (int i = 0; i < 30; i++) {
                    Thread.sleep(500);
                    if (serverReady.get()) {
                        JSObject result = new JSObject();
                        result.put("ready", true);
                        result.put("port", SERVER_PORT);
                        result.put("url", "http://127.0.0.1:" + SERVER_PORT);
                        call.resolve(result);
                        return;
                    }
                    if (nodeProcess != null && !nodeProcess.isAlive()) {
                        call.reject("Node.js process died");
                        return;
                    }
                }
                call.reject("Server timeout");
            } catch (Exception e) {
                call.reject("Error: " + e.getMessage());
            }
        }).start();
    }

    public void startServer() {
        if (nodeProcess != null && nodeProcess.isAlive()) return;
        new Thread(() -> {
            try {
                Context ctx = getContext();
                File dataDir = ctx.getFilesDir();
                File nodeDir = new File(dataDir, "nodejs");
                File serverDir = new File(dataDir, "server");
                File publicDir = new File(serverDir, "public");
                File dbDir = new File(dataDir, "data");
                File nodeBin = new File(nodeDir, "bin/node");

                nodeDir.mkdirs();
                new File(nodeDir, "bin").mkdirs();
                serverDir.mkdirs();
                publicDir.mkdirs();
                dbDir.mkdirs();

                if (!nodeBin.exists()) {
                    Log.i(TAG, "Extracting Node.js binary...");
                    extractAsset(ctx, "nodejs/bin/node", nodeBin);
                    nodeBin.setExecutable(true, false);
                    Log.i(TAG, "Node.js: " + nodeBin.length() + " bytes");
                }

                File serverJs = new File(serverDir, "server.js");
                if (!serverJs.exists() || serverJs.length() < 1000) {
                    Log.i(TAG, "Extracting server...");
                    extractAsset(ctx, "server/server.js", serverJs);
                    Log.i(TAG, "Server: " + serverJs.length() + " bytes");
                }

                File envFile = new File(serverDir, ".env");
                try {
                    extractAsset(ctx, "server/.env", envFile);
                    Log.i(TAG, ".env extracted: " + envFile.length() + " bytes");
                } catch (Exception e) {
                    Log.w(TAG, ".env not found in assets, creating default");
                    try {
                        FileWriter fw = new FileWriter(envFile);
                        fw.write("DATABASE_URL=\n");
                        fw.write("SMTP_HOST=smtp.gmail.com\n");
                        fw.write("SMTP_PORT=587\n");
                        fw.write("SMTP_SECURE=false\n");
                        fw.write("SMTP_USER=noeakili502@gmail.com\n");
                        fw.write("SMTP_PASS=\n");
                        fw.close();
                    } catch (Exception ex) { Log.e(TAG, "Failed to create .env", ex); }
                }

                if (publicDir.list() == null || publicDir.list().length == 0) {
                    Log.i(TAG, "Extracting frontend...");
                    extractAssetDir(ctx, "server/public", publicDir);
                    Log.i(TAG, "Frontend extracted.");
                }

                killExistingProcess();
                Log.i(TAG, "Starting Node.js on port " + SERVER_PORT);

                ProcessBuilder pb = new ProcessBuilder(
                    nodeBin.getAbsolutePath(),
                    "--experimental-sqlite",
                    serverJs.getAbsolutePath()
                );
                pb.directory(serverDir);
                pb.environment().put("PORT", String.valueOf(SERVER_PORT));
                pb.environment().put("NODE_ENV", "production");
                pb.environment().put("DATA_DIR", serverDir.getAbsolutePath());
                pb.redirectErrorStream(true);

                File logFile = new File(dataDir, "node_server.log");
                pb.redirectOutput(ProcessBuilder.Redirect.appendTo(logFile));

                nodeProcess = pb.start();
                Log.i(TAG, "Node.js process started, PID: " + getPid(nodeProcess));
                Log.i(TAG, "Node binary: " + nodeBin.getAbsolutePath() + " (" + nodeBin.length() + " bytes)");
                Log.i(TAG, "Server JS: " + serverJs.getAbsolutePath() + " (" + serverJs.length() + " bytes)");
                Log.i(TAG, "DATA_DIR: " + dbDir.getAbsolutePath());

                for (int i = 0; i < 30; i++) {
                    Thread.sleep(500);
                    if (nodeProcess != null && !nodeProcess.isAlive()) {
                        Log.e(TAG, "Node.js process died");
                        return;
                    }
                    if (isPortOpen(SERVER_PORT)) {
                        serverReady.set(true);
                        Log.i(TAG, "Server ready on port " + SERVER_PORT);
                        return;
                    }
                }
                Log.e(TAG, "Server timeout");
            } catch (Exception e) {
                Log.e(TAG, "Failed to start Node.js", e);
            }
        }).start();
    }

    @PluginMethod
    public void stop(PluginCall call) {
        killExistingProcess();
        serverReady.set(false);
        JSObject result = new JSObject();
        result.put("stopped", true);
        call.resolve(result);
    }

    @PluginMethod
    public void status(PluginCall call) {
        JSObject result = new JSObject();
        result.put("running", nodeProcess != null && nodeProcess.isAlive());
        result.put("ready", serverReady.get());
        result.put("port", SERVER_PORT);

        // Read last lines of server log for debugging
        try {
            File logFile = new File(getContext().getFilesDir(), "node_server.log");
            if (logFile.exists()) {
                BufferedReader reader = new BufferedReader(new FileReader(logFile));
                StringBuilder sb = new StringBuilder();
                String line;
                int lines = 0;
                while ((line = reader.readLine()) != null && lines < 20) {
                    sb.append(line).append("\n");
                    lines++;
                }
                reader.close();
                result.put("log", sb.toString());
            }
        } catch (Exception ignored) {}

        call.resolve(result);
    }

    private void killExistingProcess() {
        if (nodeProcess != null && nodeProcess.isAlive()) {
            nodeProcess.destroyForcibly();
            try { nodeProcess.waitFor(2, java.util.concurrent.TimeUnit.SECONDS); } catch (Exception ignored) {}
            nodeProcess = null;
        }
    }

    private boolean isPortOpen(int port) {
        try (Socket s = new Socket()) {
            s.connect(new InetSocketAddress("127.0.0.1", port), 200);
            return true;
        } catch (Exception e) {
            return false;
        }
    }

    private void extractAsset(Context ctx, String assetPath, File destFile) throws IOException {
        InputStream is = ctx.getAssets().open(assetPath);
        OutputStream os = new FileOutputStream(destFile);
        byte[] buf = new byte[8192];
        int read;
        while ((read = is.read(buf)) != -1) os.write(buf, 0, read);
        os.close();
        is.close();
    }

    private void extractAssetDir(Context ctx, String assetDir, File destDir) {
        try {
            String[] files = ctx.getAssets().list(assetDir);
            if (files == null) return;
            for (String file : files) {
                String path = assetDir + "/" + file;
                File dest = new File(destDir, file);
                String[] sub = ctx.getAssets().list(path);
                if (sub != null && sub.length > 0) {
                    dest.mkdirs();
                    extractAssetDir(ctx, path, dest);
                } else {
                    extractAsset(ctx, path, dest);
                }
            }
        } catch (Exception e) {
            Log.e(TAG, "extractAssetDir failed: " + assetDir, e);
        }
    }

    private long getPid(Process process) {
        try {
            Field pidField = process.getClass().getDeclaredField("pid");
            pidField.setAccessible(true);
            return pidField.getInt(process);
        } catch (Exception e) {
            return -1;
        }
    }

    @Override
    protected void handleOnDestroy() {
        killExistingProcess();
    }
}
