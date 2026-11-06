package com.nmessenger.app;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.os.Build;
import android.os.IBinder;
import android.util.Base64;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.KeyStore;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.UUID;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import javax.crypto.Cipher;
import javax.crypto.KeyGenerator;
import javax.crypto.SecretKey;
import javax.crypto.spec.GCMParameterSpec;
import org.json.JSONArray;
import org.json.JSONObject;

/** Maintains a direct authenticated connection while the user is signed in. */
public final class MessageNotificationService extends Service {
    private static final String PREFS = "nm-message-service-v1";
    private ScheduledExecutorService executor;
    private final java.util.concurrent.atomic.AtomicBoolean pollPending = new java.util.concurrent.atomic.AtomicBoolean(false);
    static { System.loadLibrary("nmessenger_client"); }
    private native void nativeStartSocket(String base, String token);
    private native void nativeStopSocket();

    private static SecretKey key() throws Exception {
        KeyStore store = KeyStore.getInstance("AndroidKeyStore"); store.load(null);
        String alias = "NMessenger.background-session.v1";
        if (!store.containsAlias(alias)) {
            KeyGenerator generator = KeyGenerator.getInstance("AES", "AndroidKeyStore");
            generator.init(new android.security.keystore.KeyGenParameterSpec.Builder(alias,
                android.security.keystore.KeyProperties.PURPOSE_ENCRYPT | android.security.keystore.KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(android.security.keystore.KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(android.security.keystore.KeyProperties.ENCRYPTION_PADDING_NONE).build());
            generator.generateKey();
        }
        return (SecretKey) store.getKey(alias, null);
    }
    private static void save(Context context, JSONObject session) throws Exception {
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding"); cipher.init(Cipher.ENCRYPT_MODE, key());
        byte[] encrypted = cipher.doFinal(session.toString().getBytes(StandardCharsets.UTF_8));
        byte[] iv = cipher.getIV(), data = new byte[iv.length + encrypted.length];
        System.arraycopy(iv, 0, data, 0, iv.length); System.arraycopy(encrypted, 0, data, iv.length, encrypted.length);
        if (!context.getSharedPreferences(PREFS, MODE_PRIVATE).edit().putString("session", Base64.encodeToString(data, Base64.NO_WRAP)).commit())
            throw new IllegalStateException("Não foi possível salvar a sessão de notificações");
    }
    private static JSONObject load(Context context) throws Exception {
        String stored = context.getSharedPreferences(PREFS, MODE_PRIVATE).getString("session", "");
        if (stored.isEmpty()) return null;
        byte[] data = Base64.decode(stored, Base64.NO_WRAP);
        if (data.length < 28) throw new SecurityException("Sessão inválida");
        Cipher cipher = Cipher.getInstance("AES/GCM/NoPadding");
        cipher.init(Cipher.DECRYPT_MODE, key(), new GCMParameterSpec(128, data, 0, 12));
        return new JSONObject(new String(cipher.doFinal(data, 12, data.length - 12), StandardCharsets.UTF_8));
    }
    public static synchronized void start(Context context, String base, String token, String account) throws Exception {
        java.net.URI uri = new java.net.URI(base);
        if ((!"https".equals(uri.getScheme()) && !("http".equals(uri.getScheme()) && ("127.0.0.1".equals(uri.getHost()) || "localhost".equals(uri.getHost()))))
            || uri.getHost() == null || uri.getUserInfo() != null || uri.getQuery() != null || uri.getFragment() != null)
            throw new SecurityException("Endereço inválido para notificações");
        SharedPreferences prefs = context.getSharedPreferences(PREFS, MODE_PRIVATE);
        if (!account.equals(prefs.getString("account", ""))) prefs.edit().clear().putString("account", account).commit();
        byte[] secret = new byte[32]; new SecureRandom().nextBytes(secret);
        save(context, new JSONObject().put("base", base).put("token", token).put("action_secret", Base64.encodeToString(secret, Base64.NO_WRAP)));
        context.startForegroundService(new Intent(context, MessageNotificationService.class));
    }
    public static synchronized void stop(Context context) {
        context.getSharedPreferences(PREFS, MODE_PRIVATE).edit().clear().commit();
        context.stopService(new Intent(context, MessageNotificationService.class));
    }
    private static synchronized void stopIfCurrent(Context context, String token) throws Exception {
        JSONObject current = load(context);
        if (current != null && token.equals(current.optString("token"))) stop(context);
    }
    public static boolean validAction(Context context, String supplied) {
        try {
            JSONObject session = load(context);
            return session != null && supplied != null && MessageDigest.isEqual(session.getString("action_secret").getBytes(StandardCharsets.UTF_8), supplied.getBytes(StandardCharsets.UTF_8));
        } catch (Exception ignored) { return false; }
    }
    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        NotificationManager manager = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
        manager.createNotificationChannel(new NotificationChannel("nmessenger_background", "Conexão de mensagens", NotificationManager.IMPORTANCE_LOW));
        Notification status = new Notification.Builder(this, "nmessenger_background")
            .setSmallIcon(getResources().getIdentifier("icon", "drawable", getPackageName()))
            .setContentTitle("NMessenger").setContentText("Recebendo notificações de mensagens")
            .setOngoing(true).build();
        if (Build.VERSION.SDK_INT >= 34) startForeground(2106, status, android.content.pm.ServiceInfo.FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING);
        else startForeground(2106, status);
        if (executor == null) {
            executor = Executors.newSingleThreadScheduledExecutor();
            // A low-frequency fallback also catches a missed event or a stale socket.
            executor.scheduleWithFixedDelay(this::onRealtimeHint, 0, 5, TimeUnit.MINUTES);
        }
        try {
            JSONObject session = load(this);
            if (session == null) { stopSelf(); return START_NOT_STICKY; }
            nativeStartSocket(session.getString("base"), session.getString("token"));
        } catch (Exception error) { stopSelf(); return START_NOT_STICKY; }
        return START_STICKY;
    }
    public void onRealtimeHint() {
        ScheduledExecutorService current = executor;
        if (current == null || !pollPending.compareAndSet(false, true)) return;
        try { current.execute(() -> { try { poll(); } finally { pollPending.set(false); } }); }
        catch (java.util.concurrent.RejectedExecutionException ignored) { pollPending.set(false); }
    }
    private void poll() {
        HttpURLConnection connection = null;
        try {
            JSONObject session = load(this);
            if (session == null) { stopSelf(); return; }
            URL url = new URL(session.getString("base") + "/notifications/unread");
            connection = (HttpURLConnection) url.openConnection();
            connection.setInstanceFollowRedirects(false); connection.setConnectTimeout(10000); connection.setReadTimeout(10000);
            connection.setRequestProperty("Authorization", "Bearer " + session.getString("token"));
            int status = connection.getResponseCode();
            if (status == 401) { stopIfCurrent(this, session.getString("token")); return; }
            if (status != 200) return;
            ByteArrayOutputStream data = new ByteArrayOutputStream();
            try (InputStream input = connection.getInputStream()) {
                byte[] buffer = new byte[4096]; int n;
                while ((n = input.read(buffer)) != -1) {
                    if (data.size() + n > 65536) throw new SecurityException("Resposta inválida");
                    data.write(buffer, 0, n);
                }
            }
            JSONArray messages = new JSONArray(data.toString("UTF-8"));
            JSONObject current = load(this);
            if (current == null || !session.getString("token").equals(current.optString("token"))) return;
            SharedPreferences prefs = getSharedPreferences(PREFS, MODE_PRIVATE);
            for (int i = 0; i < messages.length(); i++) {
                JSONObject item = messages.getJSONObject(i);
                String conversation = UUID.fromString(item.getString("conversation_id")).toString();
                String message = UUID.fromString(item.getString("message_id")).toString();
                if (!message.equals(prefs.getString("last:" + conversation, ""))) {
                    if (showMessage(conversation, message, session.getString("action_secret"))) {
                        prefs.edit().putString("last:" + conversation, message).apply();
                    }
                }
            }
        } catch (Exception ignored) {
            // The next poll retries after network outages without losing the session.
        } finally { if (connection != null) connection.disconnect(); }
    }
    private boolean showMessage(String conversation, String message, String secret) {
        NotificationManager manager = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
        if (manager == null || !manager.areNotificationsEnabled()
            || (Build.VERSION.SDK_INT >= 33 && checkSelfPermission(android.Manifest.permission.POST_NOTIFICATIONS) != android.content.pm.PackageManager.PERMISSION_GRANTED)) return false;
        manager.createNotificationChannel(new NotificationChannel("nmessenger_messages", "Mensagens", NotificationManager.IMPORTANCE_DEFAULT));
        NotificationChannel channel = manager.getNotificationChannel("nmessenger_messages");
        if (channel != null && channel.getImportance() == NotificationManager.IMPORTANCE_NONE) return false;
        Intent open = new Intent(this, MainActivity.class)
            .setAction(Intent.ACTION_MAIN)
            .addCategory(Intent.CATEGORY_LAUNCHER)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED | Intent.FLAG_ACTIVITY_SINGLE_TOP)
            .putExtra("conversation", conversation);
        PendingIntent content = PendingIntent.getActivity(this, conversation.hashCode(), open, PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        Intent read = new Intent(this, MainActivity.class).setAction("com.nmessenger.app.NOTIFICATION_READ")
            .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP | Intent.FLAG_ACTIVITY_SINGLE_TOP).putExtra("conversation", conversation).putExtra("message_id", message).putExtra("push_token", secret);
        PendingIntent readAction = PendingIntent.getActivity(this, conversation.hashCode() ^ 0x1100, read, PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        Intent reply = new Intent(this, MainActivity.class).setAction("com.nmessenger.app.NOTIFICATION_REPLY")
            .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP | Intent.FLAG_ACTIVITY_SINGLE_TOP).putExtra("conversation", conversation).putExtra("message_id", message).putExtra("push_token", secret);
        PendingIntent replyAction = PendingIntent.getActivity(this, conversation.hashCode() ^ 0x2200, reply,
            PendingIntent.FLAG_UPDATE_CURRENT | (Build.VERSION.SDK_INT >= 31 ? PendingIntent.FLAG_MUTABLE : 0));
        android.app.RemoteInput input = new android.app.RemoteInput.Builder("nm_reply").setLabel("Responder").build();
        Notification notification = new Notification.Builder(this, "nmessenger_messages")
            .setSmallIcon(getResources().getIdentifier("icon", "drawable", getPackageName()))
            .setContentTitle("NMessenger").setContentText("Você recebeu uma nova mensagem")
            .addAction(new Notification.Action.Builder(null, "Marcar como lida", readAction).build())
            .addAction(new Notification.Action.Builder(null, "Responder", replyAction).addRemoteInput(input).build())
            .setVisibility(Notification.VISIBILITY_PRIVATE).setContentIntent(content).setAutoCancel(true).build();
        manager.notify(conversation.hashCode(), notification);
        return true;
    }
    @Override public void onDestroy() { nativeStopSocket(); if (executor != null) { executor.shutdownNow(); executor = null; } super.onDestroy(); }
    @Override public IBinder onBind(Intent intent) { return null; }
}
