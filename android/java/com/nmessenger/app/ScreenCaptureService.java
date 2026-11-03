package com.nmessenger.app;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.content.pm.ServiceInfo;
import android.graphics.Bitmap;
import android.graphics.PixelFormat;
import android.hardware.display.DisplayManager;
import android.hardware.display.VirtualDisplay;
import android.media.Image;
import android.media.ImageReader;
import android.media.projection.MediaProjection;
import android.media.projection.MediaProjectionManager;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.IBinder;
import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;

/** Owns the system-approved projection and releases it when sharing ends. */
public final class ScreenCaptureService extends Service {
    private static final String STOP = "com.nmessenger.app.STOP_SCREEN";
    private MediaProjection projection;
    private VirtualDisplay display;
    private ImageReader reader;
    private HandlerThread thread;
    private long lastFrame;
    private long generation;

    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent == null || STOP.equals(intent.getAction())) { stopSelf(); return START_NOT_STICKY; }
        try {
            generation = intent.getLongExtra("generation", 0);
            NotificationManager notifications = (NotificationManager) getSystemService(NOTIFICATION_SERVICE);
            String channel = "nmessenger_screen_share";
            notifications.createNotificationChannel(new NotificationChannel(channel, "Compartilhamento de tela", NotificationManager.IMPORTANCE_LOW));
            Intent stop = new Intent(this, ScreenCaptureService.class).setAction(STOP);
            PendingIntent stopAction = PendingIntent.getService(this, 0, stop, PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
            int icon = getResources().getIdentifier("icon", "drawable", getPackageName());
            Notification notice = new Notification.Builder(this, channel).setSmallIcon(icon)
                .setContentTitle("NMessenger").setContentText("Compartilhamento de tela ativo")
                .setOngoing(true).addAction(0, "Parar", stopAction).build();
            if (android.os.Build.VERSION.SDK_INT >= 29) startForeground(2105, notice, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION);
            else startForeground(2105, notice);
            Intent permission = intent.getParcelableExtra("projection_data");
            int result = intent.getIntExtra("result_code", 0);
            MediaProjectionManager manager = (MediaProjectionManager) getSystemService(MEDIA_PROJECTION_SERVICE);
            projection = manager.getMediaProjection(result, permission);
            if (projection == null) throw new IllegalStateException("Screen capture permission unavailable");
            thread = new HandlerThread("nm-screen-capture");
            thread.start();
            Handler handler = new Handler(thread.getLooper());
            projection.registerCallback(new MediaProjection.Callback() {
                @Override public void onStop() { stopSelf(); }
            }, handler);
            android.util.DisplayMetrics metrics = getResources().getDisplayMetrics();
            float scale = Math.min(1.0f, Math.min(640.0f / metrics.widthPixels, 1280.0f / metrics.heightPixels));
            int width = Math.max(2, (int) (metrics.widthPixels * scale));
            int height = Math.max(2, (int) (metrics.heightPixels * scale));
            reader = ImageReader.newInstance(width, height, PixelFormat.RGBA_8888, 2);
            reader.setOnImageAvailableListener(source -> {
                Image image = null;
                Bitmap padded = null, frame = null;
                try {
                    image = source.acquireLatestImage();
                    if (image == null || android.os.SystemClock.elapsedRealtime() - lastFrame < 200) return;
                    lastFrame = android.os.SystemClock.elapsedRealtime();
                    Image.Plane plane = image.getPlanes()[0];
                    ByteBuffer bytes = plane.getBuffer();
                    int paddedWidth = plane.getRowStride() / plane.getPixelStride();
                    padded = Bitmap.createBitmap(paddedWidth, height, Bitmap.Config.ARGB_8888);
                    padded.copyPixelsFromBuffer(bytes);
                    frame = Bitmap.createBitmap(padded, 0, 0, width, height);
                    ByteArrayOutputStream encoded = new ByteArrayOutputStream();
                    if (frame.compress(Bitmap.CompressFormat.JPEG, 65, encoded)) MainActivity.screenFrame(generation, encoded.toByteArray());
                } catch (Throwable ignored) {
                } finally {
                    if (frame != null && frame != padded) frame.recycle();
                    if (padded != null) padded.recycle();
                    if (image != null) image.close();
                }
            }, handler);
            display = projection.createVirtualDisplay("NMessenger", width, height, metrics.densityDpi,
                DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, reader.getSurface(), null, handler);
        } catch (Throwable error) {
            stopSelf();
        }
        return START_NOT_STICKY;
    }

    @Override public void onDestroy() {
        MainActivity.screenCaptureEnded(generation);
        if (display != null) { display.release(); display = null; }
        if (reader != null) { reader.close(); reader = null; }
        if (projection != null) { projection.stop(); projection = null; }
        if (thread != null) { thread.quitSafely(); thread = null; }
        super.onDestroy();
    }

    @Override public IBinder onBind(Intent intent) { return null; }
}
