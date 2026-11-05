package com.nmessenger.app;

import android.Manifest;
import android.app.NativeActivity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Matrix;
import android.net.Uri;
import android.os.Bundle;
import android.provider.OpenableColumns;
import java.io.*;
import java.util.UUID;
import java.util.concurrent.*;

public final class MainActivity extends NativeActivity {
    public boolean musicAccessGranted() {
        android.content.ComponentName listener = new android.content.ComponentName(this, MusicNotificationListener.class);
        if (android.os.Build.VERSION.SDK_INT >= 27) {
            android.app.NotificationManager manager = (android.app.NotificationManager) getSystemService(NOTIFICATION_SERVICE);
            return manager != null && manager.isNotificationListenerAccessGranted(listener);
        }
        String enabled = android.provider.Settings.Secure.getString(getContentResolver(), "enabled_notification_listeners");
        if (enabled == null) return false;
        for (String item : enabled.split(":")) if (listener.equals(android.content.ComponentName.unflattenFromString(item))) return true;
        return false;
    }

    public void requestMusicAccess() {
        runOnUiThread(() -> startActivity(new Intent(android.provider.Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS)));
    }

    public String currentPlayingMusic() {
        if (!musicAccessGranted()) return null;
        try {
            android.media.session.MediaSessionManager manager = (android.media.session.MediaSessionManager) getSystemService(MEDIA_SESSION_SERVICE);
            if (manager == null) return null;
            for (android.media.session.MediaController player : manager.getActiveSessions(new android.content.ComponentName(this, MusicNotificationListener.class))) {
                android.media.session.PlaybackState state = player.getPlaybackState();
                if (state == null || state.getState() != android.media.session.PlaybackState.STATE_PLAYING) continue;
                android.media.MediaMetadata metadata = player.getMetadata();
                if (metadata == null) continue;
                String title = metadata.getString(android.media.MediaMetadata.METADATA_KEY_TITLE);
                if (title == null || title.trim().isEmpty()) continue;
                String artist = metadata.getString(android.media.MediaMetadata.METADATA_KEY_ARTIST);
                if (artist == null || artist.trim().isEmpty()) artist = metadata.getString(android.media.MediaMetadata.METADATA_KEY_ALBUM_ARTIST);
                return artist == null || artist.trim().isEmpty() ? title : artist + " — " + title;
            }
        } catch (SecurityException ignored) { }
        return null;
    }
    public boolean startMessageNotifications(String base, String token, String account) {
        try {
            MessageNotificationService.start(getApplicationContext(), base, token, account);
            return true;
        } catch (Exception error) { return false; }
    }

    public void stopMessageNotifications() { MessageNotificationService.stop(getApplicationContext()); }
    public String systemLocale() {
        return java.util.Locale.getDefault().toLanguageTag();
    }

    public synchronized String credential(String account, String password, boolean remove) throws Exception {
        if (!account.matches("[a-f0-9]{64}")) throw new SecurityException("Conta inválida");
        android.content.SharedPreferences preferences = getSharedPreferences("nm-secure-login", MODE_PRIVATE);
        if (remove) {
            if (!preferences.edit().remove(account).commit()) throw new IOException("Falha ao remover credencial");
            return null;
        }
        final String alias = "NMessenger.login.v1";
        java.security.KeyStore store = java.security.KeyStore.getInstance("AndroidKeyStore");
        store.load(null);
        if (!store.containsAlias(alias)) {
            if (password == null && preferences.contains(account)) throw new SecurityException("Chave do cofre indisponível");
            if (password == null) return null;
            javax.crypto.KeyGenerator generator = javax.crypto.KeyGenerator.getInstance("AES", "AndroidKeyStore");
            generator.init(new android.security.keystore.KeyGenParameterSpec.Builder(alias,
                android.security.keystore.KeyProperties.PURPOSE_ENCRYPT | android.security.keystore.KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(android.security.keystore.KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(android.security.keystore.KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256).setRandomizedEncryptionRequired(true).build());
            generator.generateKey();
        }
        javax.crypto.SecretKey key = (javax.crypto.SecretKey) store.getKey(alias, null);
        javax.crypto.Cipher cipher = javax.crypto.Cipher.getInstance("AES/GCM/NoPadding");
        byte[] aad = ("NMessenger.login.v1:" + account).getBytes(java.nio.charset.StandardCharsets.UTF_8);
        if (password != null) {
            cipher.init(javax.crypto.Cipher.ENCRYPT_MODE, key);
            cipher.updateAAD(aad);
            byte[] clear = password.getBytes(java.nio.charset.StandardCharsets.UTF_8);
            byte[] encrypted;
            try { encrypted = cipher.doFinal(clear); }
            finally { java.util.Arrays.fill(clear, (byte) 0); }
            byte[] iv = cipher.getIV();
            byte[] value = new byte[iv.length + encrypted.length];
            System.arraycopy(iv, 0, value, 0, iv.length);
            System.arraycopy(encrypted, 0, value, iv.length, encrypted.length);
            if (!preferences.edit().putString(account, android.util.Base64.encodeToString(value, android.util.Base64.NO_WRAP)).commit()) throw new IOException("Falha ao salvar credencial protegida");
            return null;
        }
        String stored = preferences.getString(account, null);
        if (stored == null) return null;
        if (stored.length() > 16384) throw new SecurityException("Credencial inválida");
        byte[] value = android.util.Base64.decode(stored, android.util.Base64.NO_WRAP);
        if (value.length < 28) throw new SecurityException("Credencial inválida");
        cipher.init(javax.crypto.Cipher.DECRYPT_MODE, key, new javax.crypto.spec.GCMParameterSpec(128, value, 0, 12));
        cipher.updateAAD(aad);
        byte[] clear = cipher.doFinal(value, 12, value.length - 12);
        try { return new String(clear, java.nio.charset.StandardCharsets.UTF_8); }
        finally { java.util.Arrays.fill(clear, (byte) 0); }
    }
    static {
        try {
            System.loadLibrary("nmessenger_client");
        } catch (Throwable ignored) {}
    }

    private static final int DOCUMENT = 2101, MICROPHONE = 2102, CAMERA = 2103, NOTIFICATIONS = 2104, SCREEN_CAPTURE = 2105;
    private final ConcurrentHashMap<String, Uri> saves = new ConcurrentHashMap<>();
    private volatile boolean savingDocument;
    private volatile String suggestedName;
    private volatile CountDownLatch microphoneLatch;
    private volatile CountDownLatch cameraPermissionLatch;
    private String[] pendingNotification;
    private boolean notificationPermissionPending;
    private volatile String notificationConversation;
    private volatile String notificationAction;
    private android.os.HandlerThread cameraThread;
    private android.os.Handler cameraHandler;
    private android.hardware.Camera camera;
    private android.graphics.SurfaceTexture cameraSurface;
    private static native void nativeCameraFrame(byte[] jpeg);
    private static native void nativeScreenFrame(byte[] jpeg);
    private static volatile int screenCaptureState;
    private static volatile long screenCaptureGeneration;
    private static volatile int screenCaptureRequest;

    static void screenFrame(long generation, byte[] jpeg) {
        if (screenCaptureState == 2 && screenCaptureGeneration == generation) nativeScreenFrame(jpeg);
    }
    static void screenCaptureEnded(long generation) {
        synchronized (MainActivity.class) {
            if (screenCaptureState != 0 && screenCaptureGeneration == generation) screenCaptureState = 3;
        }
    }

    public boolean startScreenCapture() {
        synchronized (MainActivity.class) {
            if (screenCaptureState != 0) return false;
            screenCaptureGeneration++;
            screenCaptureRequest = SCREEN_CAPTURE + (int) (screenCaptureGeneration % 20000);
            screenCaptureState = 1;
        }
        final int request = screenCaptureRequest;
        runOnUiThread(() -> {
            try {
                android.media.projection.MediaProjectionManager manager =
                    (android.media.projection.MediaProjectionManager) getSystemService(MEDIA_PROJECTION_SERVICE);
                if (screenCaptureState == 1 && screenCaptureRequest == request)
                    startActivityForResult(manager.createScreenCaptureIntent(), request);
            } catch (Throwable error) { screenCaptureState = 3; }
        });
        return true;
    }

    public int screenCaptureState() { return screenCaptureState; }

    public void stopScreenCapture() {
        synchronized (MainActivity.class) {
            screenCaptureState = 0;
            stopService(new Intent(this, ScreenCaptureService.class));
        }
    }
    private static native void nativeDocumentResult(String path, String error);
    private static native void nativeKeyboardHeight(int heightPx, int heightDp);
    private static native void nativeStatusBarHeight(int heightDp);

    private volatile int currentKeyboardHeightPx = 0;
    private volatile int currentKeyboardHeightDp = 0;
    private volatile int currentStatusBarHeightDp = 0;
    private volatile int currentNavigationBarHeightDp = 0;

    public int keyboardHeightPx() {
        return currentKeyboardHeightPx;
    }

    public int keyboardHeightDp() {
        return currentKeyboardHeightDp;
    }

    public int statusBarHeightDp() {
        if (currentStatusBarHeightDp > 0) return currentStatusBarHeightDp;
        updateStatusBarHeight(null);
        return Math.max(currentStatusBarHeightDp, 36);
    }

    public int navigationBarHeightDp() {
        if (currentNavigationBarHeightDp > 0) return currentNavigationBarHeightDp;
        updateNavigationBarHeight(null);
        return currentNavigationBarHeightDp;
    }

    private void updateStatusBarHeight(android.view.WindowInsets insets) {
        int heightPx = 0;
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
            try {
                if (insets == null) {
                    insets = getWindow().getDecorView().getRootWindowInsets();
                }
                if (insets != null) {
                    android.graphics.Insets sb = insets.getInsets(
                        android.view.WindowInsets.Type.statusBars() | android.view.WindowInsets.Type.displayCutout()
                    );
                    heightPx = sb.top;
                }
            } catch (Throwable ignored) {}
        }
        if (heightPx <= 0) {
            try {
                int resId = getResources().getIdentifier("status_bar_height", "dimen", "android");
                if (resId > 0) {
                    heightPx = getResources().getDimensionPixelSize(resId);
                }
            } catch (Throwable ignored) {}
        }
        if (heightPx <= 0) {
            try {
                android.graphics.Rect r = new android.graphics.Rect();
                getWindow().getDecorView().getWindowVisibleDisplayFrame(r);
                heightPx = r.top;
            } catch (Throwable ignored) {}
        }
        float density = getResources().getDisplayMetrics().density;
        int dp = (int) Math.ceil(heightPx / (density > 0.0f ? density : 1.0f));
        if (dp > 0) {
            currentStatusBarHeightDp = dp;
            try {
                nativeStatusBarHeight(dp);
            } catch (Throwable ignored) {}
        }
    }

    private void updateNavigationBarHeight(android.view.WindowInsets insets) {
        int heightPx = 0;
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
            try {
                if (insets == null) insets = getWindow().getDecorView().getRootWindowInsets();
                if (insets != null) {
                    heightPx = insets.getInsets(android.view.WindowInsets.Type.navigationBars()).bottom;
                }
            } catch (Throwable ignored) {}
        }
        if (heightPx <= 0) {
            try {
                int resId = getResources().getIdentifier("navigation_bar_height", "dimen", "android");
                if (resId > 0) heightPx = getResources().getDimensionPixelSize(resId);
            } catch (Throwable ignored) {}
        }
        float density = getResources().getDisplayMetrics().density;
        int dp = (int) Math.ceil(heightPx / (density > 0.0f ? density : 1.0f));
        currentNavigationBarHeightDp = dp;
    }

    private void updateKeyboardHeight(android.view.WindowInsets insets) {
        int heightPx = 0;
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
            try {
                if (insets == null) {
                    insets = getWindow().getDecorView().getRootWindowInsets();
                }
                if (insets != null) {
                    heightPx = insets.getInsets(android.view.WindowInsets.Type.ime()).bottom;
                }
            } catch (Throwable ignored) {}
        }
        if (heightPx <= 0) {
            try {
                android.graphics.Rect r = new android.graphics.Rect();
                getWindow().getDecorView().getWindowVisibleDisplayFrame(r);
                int totalHeight;
                if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                    android.graphics.Rect bounds = getWindowManager().getCurrentWindowMetrics().getBounds();
                    totalHeight = bounds.height();
                } else {
                    android.util.DisplayMetrics dm = new android.util.DisplayMetrics();
                    getWindowManager().getDefaultDisplay().getRealMetrics(dm);
                    totalHeight = dm.heightPixels;
                }
                int keypadHeight = totalHeight - r.bottom;
                if (keypadHeight > totalHeight * 0.15) {
                    heightPx = keypadHeight;
                }
            } catch (Throwable ignored) {}
        }
        float density = getResources().getDisplayMetrics().density;
        int dp = (int) Math.ceil(heightPx / (density > 0.0f ? density : 1.0f));
        currentKeyboardHeightPx = heightPx;
        currentKeyboardHeightDp = dp;
        try {
            nativeKeyboardHeight(heightPx, dp);
        } catch (Throwable ignored) {}
    }

    private ImeInputView imeInputView;

    public native void nativeCommitText(String text);
    public native void nativeComposingText(String text);
    public native void nativeDeleteSurroundingText(int before, int after);
    public native void nativeBackFromKeyboard();
    public native void nativeEditAction(int action);

    public void onImeCommitText(String text) {
        try {
            nativeCommitText(text);
        } catch (Throwable ignored) {}
    }

    public void onImeDeleteSurroundingText(int before, int after) {
        try {
            nativeDeleteSurroundingText(before, after);
        } catch (Throwable ignored) {}
    }

    private volatile boolean keyboardRequested = false;
    private volatile String currentInputType = "text";

    public void setKeyboardType(String type) {
        if (type == null || type.isEmpty()) type = "text";
        final String newType = type;
        if (newType.equals(currentInputType)) {
            return;
        }
        currentInputType = newType;
        runOnUiThread(() -> {
            try {
                if (imeInputView != null) {
                    if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
                        if ("email".equals(newType)) {
                            imeInputView.setAutofillHints(
                                android.view.View.AUTOFILL_HINT_EMAIL_ADDRESS,
                                android.view.View.AUTOFILL_HINT_USERNAME
                            );
                            imeInputView.setImportantForAutofill(android.view.View.IMPORTANT_FOR_AUTOFILL_YES);
                        } else if ("password".equals(newType)) {
                            imeInputView.setAutofillHints(android.view.View.AUTOFILL_HINT_PASSWORD);
                            imeInputView.setImportantForAutofill(android.view.View.IMPORTANT_FOR_AUTOFILL_YES);
                        } else {
                            imeInputView.setImportantForAutofill(android.view.View.IMPORTANT_FOR_AUTOFILL_NO);
                        }
                    }
                    if (keyboardRequested) {
                        android.view.inputmethod.InputMethodManager imm =
                            (android.view.inputmethod.InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
                        if (imm != null) {
                            imm.restartInput(imeInputView);
                            imm.showSoftInput(imeInputView, 0);
                        }
                    }
                }
            } catch (Throwable ignored) {}
        });
    }

    static class ImeInputView extends android.view.View {
        private final MainActivity activity;

        public ImeInputView(MainActivity activity) {
            super(activity);
            this.activity = activity;
            setFocusable(true);
            setFocusableInTouchMode(true);
            setVisibility(android.view.View.VISIBLE);
            setAlpha(0.0f);
        }

        @Override
        public boolean onCheckIsTextEditor() {
            return true;
        }

        @Override public int getAutofillType() {
            return ("email".equals(activity.currentInputType) || "password".equals(activity.currentInputType))
                ? android.view.View.AUTOFILL_TYPE_TEXT : android.view.View.AUTOFILL_TYPE_NONE;
        }

        @Override public void autofill(android.view.autofill.AutofillValue value) {
            if (value != null && value.isText() && value.getTextValue() != null) {
                activity.nativeEditAction(4);
                activity.onImeCommitText(value.getTextValue().toString());
            }
        }

        @Override
        protected void onFocusChanged(boolean gainFocus, int direction, android.graphics.Rect previouslyFocusedRect) {
            super.onFocusChanged(gainFocus, direction, previouslyFocusedRect);
            if (!gainFocus && activity.keyboardRequested) {
                post(() -> {
                    if (activity.keyboardRequested && !isFocused()) {
                        requestFocus();
                    }
                });
            }
        }

        @Override
        public android.view.inputmethod.InputConnection onCreateInputConnection(android.view.inputmethod.EditorInfo outAttrs) {
            String type = activity.currentInputType;
            if ("email".equals(type)) {
                outAttrs.inputType = android.text.InputType.TYPE_CLASS_TEXT
                    | android.text.InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS;
            } else if ("password".equals(type)) {
                outAttrs.inputType = android.text.InputType.TYPE_CLASS_TEXT
                    | android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD;
            } else {
                outAttrs.inputType = android.text.InputType.TYPE_CLASS_TEXT
                    | android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE | android.text.InputType.TYPE_TEXT_FLAG_CAP_SENTENCES;
            }
            outAttrs.imeOptions = android.view.inputmethod.EditorInfo.IME_FLAG_NO_EXTRACT_UI
                | android.view.inputmethod.EditorInfo.IME_FLAG_NO_FULLSCREEN;

            return new android.view.inputmethod.BaseInputConnection(this, true) {
                private String composing = "";
                @Override
                public boolean setComposingText(CharSequence text, int newCursorPosition) {
                    composing = text == null ? "" : text.toString();
                    activity.nativeComposingText(composing);
                    return super.setComposingText(text, newCursorPosition);
                }

                @Override
                public boolean finishComposingText() {
                    if (!composing.isEmpty()) {
                        activity.onImeCommitText(composing);
                        composing = "";
                    }
                    return super.finishComposingText();
                }

                @Override
                public boolean commitText(CharSequence text, int newCursorPosition) {
                    if (text != null && text.length() > 0) {
                        activity.onImeCommitText(text.toString());
                    }
                    composing = "";
                    return super.commitText(text, newCursorPosition);
                }

                @Override
                public boolean deleteSurroundingText(int beforeLength, int afterLength) {
                    activity.onImeDeleteSurroundingText(beforeLength, afterLength);
                    return super.deleteSurroundingText(beforeLength, afterLength);
                }

                @Override
                public boolean performEditorAction(int actionCode) {
                    activity.onImeCommitText("\n");
                    return true;
                }

                @Override
                public boolean sendKeyEvent(android.view.KeyEvent event) {
                    int code = event.getKeyCode();
                    if (event.getAction() == android.view.KeyEvent.ACTION_DOWN) {
                        if (code == android.view.KeyEvent.KEYCODE_DEL) {
                            activity.onImeDeleteSurroundingText(1, 0);
                            return true;
                        } else if (code == android.view.KeyEvent.KEYCODE_ENTER) {
                            activity.onImeCommitText("\n");
                            return true;
                        } else {
                            int unicode = event.getUnicodeChar();
                            if (unicode != 0) {
                                activity.onImeCommitText(String.valueOf((char) unicode));
                                return true;
                            }
                        }
                    } else if (event.getAction() == android.view.KeyEvent.ACTION_UP) {
                        if (code == android.view.KeyEvent.KEYCODE_DEL || code == android.view.KeyEvent.KEYCODE_ENTER) {
                            return true;
                        }
                    }
                    return super.sendKeyEvent(event);
                }

                @Override
                public boolean performContextMenuAction(int id) {
                    if (id == android.R.id.selectAll) { activity.nativeEditAction(4); return true; }
                    if (id == android.R.id.copy) { activity.nativeEditAction(1); return true; }
                    if (id == android.R.id.cut) { activity.nativeEditAction(2); return true; }
                    if (id == android.R.id.paste) {
                        String text = activity.getClipboardText();
                        if (text != null && !text.isEmpty()) {
                            activity.onImeCommitText(text);
                            return true;
                        }
                    }
                    return super.performContextMenuAction(id);
                }
            };
        }
    }

    public void showKeyboard(String type) {
        setKeyboardType(type);
        showKeyboard();
    }

    public void showKeyboard() {
        keyboardRequested = true;
        runOnUiThread(() -> {
            try {
                if (imeInputView != null) {
                    imeInputView.setFocusable(true);
                    imeInputView.setFocusableInTouchMode(true);
                    if (!imeInputView.isFocused()) {
                        imeInputView.requestFocus();
                    }
                    if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                        android.view.WindowInsetsController wic = getWindow().getInsetsController();
                        if (wic != null) {
                            wic.show(android.view.WindowInsets.Type.ime());
                        }
                    }
                    android.view.inputmethod.InputMethodManager imm =
                        (android.view.inputmethod.InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
                    if (imm != null) {
                        imm.showSoftInput(imeInputView, 0);
                    }
                    imeInputView.post(() -> {
                        if (keyboardRequested && imeInputView != null) {
                            android.view.inputmethod.InputMethodManager imm2 =
                                (android.view.inputmethod.InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
                            if (imm2 != null) {
                                imm2.showSoftInput(imeInputView, 0);
                            }
                        }
                    });
                }
            } catch (Throwable ignored) {}
        });
    }

    public void hideKeyboard() {
        keyboardRequested = false;
        runOnUiThread(() -> {
            try {
                if (!keyboardRequested) {
                    if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                        android.view.WindowInsetsController wic = getWindow().getInsetsController();
                        if (wic != null) {
                            wic.hide(android.view.WindowInsets.Type.ime());
                        }
                    }
                    android.view.inputmethod.InputMethodManager imm =
                        (android.view.inputmethod.InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
                    if (imm != null) {
                        if (imeInputView != null) {
                            imm.hideSoftInputFromWindow(imeInputView.getWindowToken(), 0);
                        }
                        imm.hideSoftInputFromWindow(getWindow().getDecorView().getWindowToken(), 0);
                    }
                    if (imeInputView != null) {
                        imeInputView.clearFocus();
                    }
                }
            } catch (Throwable ignored) {}
        });
    }

    @Override
    public boolean dispatchKeyEvent(android.view.KeyEvent event) {
        if (event.getKeyCode() == android.view.KeyEvent.KEYCODE_BACK) {
            if (event.getAction() == android.view.KeyEvent.ACTION_UP) {
                if (currentKeyboardHeightPx > 0 || keyboardRequested) {
                    hideKeyboard();
                    try {
                        nativeBackFromKeyboard();
                    } catch (Throwable ignored) {}
                    return true;
                }
            }
        }
        return super.dispatchKeyEvent(event);
    }

    @Override
    public void onBackPressed() {
        if (currentKeyboardHeightPx > 0 || keyboardRequested) {
            hideKeyboard();
            try {
                nativeBackFromKeyboard();
            } catch (Throwable ignored) {}
            return;
        }
        super.onBackPressed();
    }

    public String getClipboardText() {
        try {
            android.content.ClipboardManager clipboard =
                (android.content.ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
            if (clipboard != null && clipboard.hasPrimaryClip()) {
                android.content.ClipData clip = clipboard.getPrimaryClip();
                if (clip != null && clip.getItemCount() > 0) {
                    CharSequence text = clip.getItemAt(0).coerceToText(this);
                    if (text != null && text.length() > 0) return text.toString();
                }
            }
        } catch (Throwable ignored) {}
        return null;
    }

    public void setClipboardText(String text) {
        if (text == null) return;
        runOnUiThread(() -> {
            try {
                android.content.ClipboardManager clipboard =
                    (android.content.ClipboardManager) getSystemService(CLIPBOARD_SERVICE);
                if (clipboard != null) {
                    android.content.ClipData clip =
                        android.content.ClipData.newPlainText("NMessenger", text);
                    clipboard.setPrimaryClip(clip);
                }
            } catch (Throwable ignored) {}
        });
    }

    public void attention() {
        android.os.Vibrator vibrator = (android.os.Vibrator) getSystemService(VIBRATOR_SERVICE);
        if (vibrator != null && vibrator.hasVibrator()) vibrator.vibrate(android.os.VibrationEffect.createOneShot(180, 100));
    }

    public void notifyMessage(String title, String body, String conversation) {
        runOnUiThread(() -> {
            if (android.os.Build.VERSION.SDK_INT >= 33
                && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
                pendingNotification = new String[]{title, body, conversation};
                if (!notificationPermissionPending) {
                    notificationPermissionPending = true;
                    requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS}, NOTIFICATIONS);
                }
                return;
            }
            postMessageNotification(title, body, conversation);
        });
    }

    private void postMessageNotification(String title, String body, String conversation) {
        android.app.NotificationManager manager = (android.app.NotificationManager) getSystemService(NOTIFICATION_SERVICE);
        if (manager == null) return;
        String channel = "nmessenger_messages";
        manager.createNotificationChannel(new android.app.NotificationChannel(channel, "Mensagens", android.app.NotificationManager.IMPORTANCE_DEFAULT));
        Intent intent = new Intent(this, MainActivity.class).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP | Intent.FLAG_ACTIVITY_SINGLE_TOP);
        intent.putExtra("conversation", conversation);
        int id = conversation.isEmpty() ? title.hashCode() : conversation.hashCode();
        android.app.PendingIntent action = android.app.PendingIntent.getActivity(this, id, intent,
            android.app.PendingIntent.FLAG_UPDATE_CURRENT | android.app.PendingIntent.FLAG_IMMUTABLE);
        int icon = getResources().getIdentifier("icon", "drawable", getPackageName());
        android.app.Notification notification = new android.app.Notification.Builder(this, channel)
            .setSmallIcon(icon).setContentTitle(title).setContentText(body)
            .setStyle(new android.app.Notification.BigTextStyle().bigText(body))
            .setVisibility(android.app.Notification.VISIBILITY_PRIVATE)
            .setContentIntent(action).setAutoCancel(true).build();
        manager.notify(id, notification);
    }

    public String takeNotificationConversation() {
        String conversation = notificationConversation;
        notificationConversation = null;
        return conversation;
    }

    public String takeNotificationAction() {
        String result = notificationAction;
        notificationAction = null;
        return result;
    }

    private void receiveNotificationIntent(Intent intent) {
        if (intent == null) return;
        String conversation = intent.getStringExtra("conversation");
        notificationConversation = conversation;
        String action = intent.getAction();
        if (conversation == null || action == null) return;
        if (("com.nmessenger.app.NOTIFICATION_READ".equals(action) || "com.nmessenger.app.NOTIFICATION_REPLY".equals(action))
            && !MessageNotificationService.validAction(this, intent.getStringExtra("push_token"))) return;
        try {
            if ("com.nmessenger.app.NOTIFICATION_READ".equals(action)) {
                notificationAction = new org.json.JSONObject().put("type", "read").put("conversation", conversation)
                    .put("message", intent.getStringExtra("message_id")).toString();
            } else if ("com.nmessenger.app.NOTIFICATION_REPLY".equals(action)) {
                android.os.Bundle input = android.app.RemoteInput.getResultsFromIntent(intent);
                CharSequence reply = input == null ? null : input.getCharSequence("nm_reply");
                if (reply != null && reply.length() > 0) notificationAction = new org.json.JSONObject()
                    .put("type", "reply").put("conversation", conversation).put("text", reply.toString()).toString();
            }
        } catch (org.json.JSONException ignored) { }
    }

    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        receiveNotificationIntent(intent);
    }

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        if (android.os.Build.VERSION.SDK_INT >= 33
            && checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
            && !getPreferences(MODE_PRIVATE).getBoolean("notifications_requested", false)) {
            getPreferences(MODE_PRIVATE).edit().putBoolean("notifications_requested", true).apply();
            notificationPermissionPending = true;
            requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS}, NOTIFICATIONS);
        }
        receiveNotificationIntent(getIntent());
        getWindow().setStatusBarColor(0xffc2ddf4);
        getWindow().setNavigationBarColor(0xffeff8fa);
        getWindow().getDecorView().setSystemUiVisibility(android.view.View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR);

        imeInputView = new ImeInputView(this);
        addContentView(imeInputView, new android.view.ViewGroup.LayoutParams(1, 1));

        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
            getWindow().getDecorView().setOnApplyWindowInsetsListener((v, insets) -> {
                updateKeyboardHeight(insets);
                updateStatusBarHeight(insets);
                updateNavigationBarHeight(insets);
                return v.onApplyWindowInsets(insets);
            });
            try {
                getWindow().getDecorView().setWindowInsetsAnimationCallback(
                    new android.view.WindowInsetsAnimation.Callback(
                        android.view.WindowInsetsAnimation.Callback.DISPATCH_MODE_STOP
                    ) {
                        @Override
                        public android.view.WindowInsets onProgress(
                            android.view.WindowInsets insets,
                            java.util.List<android.view.WindowInsetsAnimation> runningAnimations
                        ) {
                            updateKeyboardHeight(insets);
                            updateStatusBarHeight(insets);
                            updateNavigationBarHeight(insets);
                            return insets;
                        }
                    }
                );
            } catch (Throwable ignored) {}
        }

        getWindow().getDecorView().getViewTreeObserver().addOnGlobalLayoutListener(() -> {
            updateKeyboardHeight(null);
            updateStatusBarHeight(null);
            updateNavigationBarHeight(null);
        });
        updateStatusBarHeight(null);
        updateNavigationBarHeight(null);
    }

    @Override public void onConfigurationChanged(android.content.res.Configuration newConfig) {
        super.onConfigurationChanged(newConfig);
        updateKeyboardHeight(null);
        updateStatusBarHeight(null);
        updateNavigationBarHeight(null);
    }

    public void selectDocument(String mime, boolean save, String name) {
        savingDocument = save;
        suggestedName = name;
        runOnUiThread(() -> {
            try {
                Intent intent = new Intent(save ? Intent.ACTION_CREATE_DOCUMENT : Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType(mime);
                if (save) intent.putExtra(Intent.EXTRA_TITLE, name);
                startActivityForResult(intent, DOCUMENT);
            } catch (Exception error) { nativeDocumentResult(null, error.toString()); }
        });
    }

    @Override protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == screenCaptureRequest) {
            synchronized (MainActivity.class) {
                if (screenCaptureState != 1) return;
                if (result != RESULT_OK || data == null) { screenCaptureState = 3; return; }
                try {
                    Intent service = new Intent(this, ScreenCaptureService.class)
                        .putExtra("result_code", result).putExtra("projection_data", data)
                        .putExtra("generation", screenCaptureGeneration);
                    screenCaptureState = 2;
                    startForegroundService(service);
                } catch (Throwable error) { screenCaptureState = 3; }
            }
            return;
        }
        if (request != DOCUMENT) return;
        if (result != RESULT_OK || data == null || data.getData() == null) {
            nativeDocumentResult(null, null);
            return;
        }
        final Uri uri = data.getData();
        final boolean save = savingDocument;
        new Thread(() -> {
            try {
                String name = suggestedName;
                if (!save) {
                    try (Cursor cursor = getContentResolver().query(uri, new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
                        if (cursor != null && cursor.moveToFirst()) name = cursor.getString(0);
                    }
                }
                if (name == null || name.trim().isEmpty()) name = "arquivo";
                name = name.replaceAll("[\\\\/\\p{Cntrl}]", "_");
                if (name.equals(".") || name.equals("..")) name = "arquivo";
                File directory = new File(getCacheDir(), "document-" + UUID.randomUUID());
                if (!directory.mkdirs()) throw new IOException("Não foi possível preparar o arquivo");
                File file = new File(directory, name);
                if (save) saves.put(file.getAbsolutePath(), uri);
                else {
                    try (InputStream input = getContentResolver().openInputStream(uri); OutputStream output = new FileOutputStream(file)) {
                        if (input == null) throw new IOException("Arquivo indisponível");
                        copy(input, output, 25L * 1024 * 1024);
                    }
                }
                nativeDocumentResult(file.getAbsolutePath(), null);
            } catch (Exception error) { nativeDocumentResult(null, error.toString()); }
        }, "nm-document").start();
    }

    public String commitDocument(String path) {
        Uri uri = saves.remove(path);
        if (uri == null) return "Destino do arquivo não encontrado";
        try (InputStream input = new FileInputStream(path); OutputStream output = getContentResolver().openOutputStream(uri, "w")) {
            if (output == null) return "Não foi possível abrir o destino";
            copy(input, output, Long.MAX_VALUE);
            new File(path).delete();
            return null;
        } catch (Exception error) { return error.toString(); }
    }

    private static void copy(InputStream input, OutputStream output, long maximum) throws IOException {
        byte[] buffer = new byte[16384];
        long total = 0;
        for (int count; (count = input.read(buffer)) != -1;) {
            total += count;
            if (total > maximum) throw new IOException("O limite por arquivo é 25 MiB");
            output.write(buffer, 0, count);
        }
    }

    public boolean ensureMicrophone() {
        if (checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED) return true;
        final CountDownLatch latch;
        synchronized (this) {
            if (microphoneLatch == null) {
                microphoneLatch = new CountDownLatch(1);
                runOnUiThread(() -> requestPermissions(new String[]{Manifest.permission.RECORD_AUDIO}, MICROPHONE));
            }
            latch = microphoneLatch;
        }
        try { latch.await(60, TimeUnit.SECONDS); }
        catch (InterruptedException error) { Thread.currentThread().interrupt(); }
        return checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED;
    }

    public boolean ensureCamera() {
        if (checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) return true;
        final CountDownLatch latch;
        synchronized (this) {
            if (cameraPermissionLatch == null) {
                cameraPermissionLatch = new CountDownLatch(1);
                runOnUiThread(() -> requestPermissions(new String[]{Manifest.permission.CAMERA}, CAMERA));
            }
            latch = cameraPermissionLatch;
        }
        try { latch.await(60, TimeUnit.SECONDS); }
        catch (InterruptedException error) { Thread.currentThread().interrupt(); }
        return checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED;
    }

    public boolean startCamera() {
        if (!ensureCamera()) return false;
        final android.os.Handler handler;
        synchronized (this) {
            if (cameraThread != null) return false;
            cameraThread = new android.os.HandlerThread("nm-camera");
            cameraThread.start();
            cameraHandler = new android.os.Handler(cameraThread.getLooper());
            handler = cameraHandler;
        }
        CountDownLatch started = new CountDownLatch(1);
        final boolean[] success = { false };
        handler.post(() -> {
            try {
                int selected = 0;
                boolean isFront = false;
                int sensorOrientation = 0;
                for (int i = 0; i < android.hardware.Camera.getNumberOfCameras(); i++) {
                    android.hardware.Camera.CameraInfo info = new android.hardware.Camera.CameraInfo();
                    android.hardware.Camera.getCameraInfo(i, info);
                    if (info.facing == android.hardware.Camera.CameraInfo.CAMERA_FACING_FRONT) {
                        selected = i;
                        isFront = true;
                        sensorOrientation = info.orientation;
                        break;
                    }
                }
                if (!isFront && android.hardware.Camera.getNumberOfCameras() > 0) {
                    android.hardware.Camera.CameraInfo info = new android.hardware.Camera.CameraInfo();
                    android.hardware.Camera.getCameraInfo(selected, info);
                    sensorOrientation = info.orientation;
                    isFront = (info.facing == android.hardware.Camera.CameraInfo.CAMERA_FACING_FRONT);
                }
                final boolean frontFacing = isFront;
                final int cameraSensorOrientation = sensorOrientation;

                camera = android.hardware.Camera.open(selected);
                android.hardware.Camera.Parameters params = camera.getParameters();
                android.hardware.Camera.Size chosen = null;
                for (android.hardware.Camera.Size size : params.getSupportedPreviewSizes()) {
                    if (size.width <= 640 && size.height <= 480 && (chosen == null || size.width * size.height > chosen.width * chosen.height)) chosen = size;
                }
                if (chosen != null) params.setPreviewSize(chosen.width, chosen.height);
                params.setPreviewFormat(android.graphics.ImageFormat.NV21);
                camera.setParameters(params);
                android.hardware.Camera.Size size = camera.getParameters().getPreviewSize();
                cameraSurface = new android.graphics.SurfaceTexture(false);
                camera.setPreviewTexture(cameraSurface);
                final long[] lastFrame = { 0 };
                camera.setPreviewCallback((data, source) -> {
                    long now = android.os.SystemClock.elapsedRealtime();
                    if (data == null || now - lastFrame[0] < 100) return;
                    lastFrame[0] = now;
                    try {
                        ByteArrayOutputStream buffer = new ByteArrayOutputStream();
                        android.graphics.YuvImage frame = new android.graphics.YuvImage(data, android.graphics.ImageFormat.NV21, size.width, size.height, null);
                        if (frame.compressToJpeg(new android.graphics.Rect(0, 0, size.width, size.height), 70, buffer)) {
                            byte[] rawJpeg = buffer.toByteArray();
                            int rotation = getCameraOrientation(frontFacing, cameraSensorOrientation);
                            if (rotation != 0 || frontFacing) {
                                Bitmap bmp = BitmapFactory.decodeByteArray(rawJpeg, 0, rawJpeg.length);
                                if (bmp != null) {
                                    Matrix matrix = new Matrix();
                                    if (rotation != 0) matrix.postRotate(rotation);
                                    if (frontFacing) matrix.postScale(-1.0f, 1.0f);
                                    Bitmap rotated = Bitmap.createBitmap(
                                        bmp, 0, 0, bmp.getWidth(), bmp.getHeight(), matrix, true
                                    );
                                    ByteArrayOutputStream out = new ByteArrayOutputStream();
                                    rotated.compress(Bitmap.CompressFormat.JPEG, 65, out);
                                    nativeCameraFrame(out.toByteArray());
                                    bmp.recycle();
                                    if (rotated != bmp) rotated.recycle();
                                } else {
                                    nativeCameraFrame(rawJpeg);
                                }
                            } else {
                                nativeCameraFrame(rawJpeg);
                            }
                        }
                    } catch (Throwable ignored) {}
                });
                camera.startPreview();
                success[0] = true;
            } catch (Throwable error) {
                if (camera != null) { camera.release(); camera = null; }
                if (cameraSurface != null) { cameraSurface.release(); cameraSurface = null; }
            } finally { started.countDown(); }
        });
        try { started.await(10, TimeUnit.SECONDS); }
        catch (InterruptedException error) { Thread.currentThread().interrupt(); }
        if (!success[0]) stopCamera();
        return success[0];
    }

    public synchronized void stopCamera() {
        if (cameraThread == null) return;
        android.os.HandlerThread thread = cameraThread;
        android.os.Handler handler = cameraHandler;
        cameraThread = null;
        cameraHandler = null;
        CountDownLatch stopped = new CountDownLatch(1);
        handler.post(() -> {
            try {
                if (camera != null) {
                    try { camera.setPreviewCallback(null); camera.stopPreview(); } catch (Throwable ignored) {}
                    camera.release(); camera = null;
                }
                if (cameraSurface != null) { cameraSurface.release(); cameraSurface = null; }
            } finally { thread.quitSafely(); stopped.countDown(); }
        });
        try { stopped.await(3, TimeUnit.SECONDS); }
        catch (InterruptedException error) { Thread.currentThread().interrupt(); }
    }

    private int getCameraOrientation(boolean isFront, int sensorOrientation) {
        int degrees = 0;
        try {
            int rotation = getWindowManager().getDefaultDisplay().getRotation();
            switch (rotation) {
                case android.view.Surface.ROTATION_0: degrees = 0; break;
                case android.view.Surface.ROTATION_90: degrees = 90; break;
                case android.view.Surface.ROTATION_180: degrees = 180; break;
                case android.view.Surface.ROTATION_270: degrees = 270; break;
            }
        } catch (Throwable ignored) {}
        if (isFront) {
            return (sensorOrientation - degrees + 360) % 360;
        } else {
            return (sensorOrientation + degrees) % 360;
        }
    }

    @Override public void onRequestPermissionsResult(int request, String[] permissions, int[] results) {
        super.onRequestPermissionsResult(request, permissions, results);
        if (request == MICROPHONE) {
            synchronized (this) {
                if (microphoneLatch != null) microphoneLatch.countDown();
                microphoneLatch = null;
            }
        }
        if (request == CAMERA) {
            synchronized (this) {
                if (cameraPermissionLatch != null) cameraPermissionLatch.countDown();
                cameraPermissionLatch = null;
            }
        }
        if (request == NOTIFICATIONS) {
            notificationPermissionPending = false;
            String[] pending = pendingNotification;
            pendingNotification = null;
            if (pending != null && results.length > 0 && results[0] == PackageManager.PERMISSION_GRANTED)
                postMessageNotification(pending[0], pending[1], pending[2]);
        }
    }
}
