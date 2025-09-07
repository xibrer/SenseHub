package com.example.androidclient


import android.Manifest
import android.app.AlertDialog
import android.content.pm.PackageManager
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Build
import android.os.Bundle
import android.util.Base64
import android.util.Log
import org.json.JSONObject
import android.widget.Button
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import org.eclipse.paho.client.mqttv3.IMqttActionListener
import org.eclipse.paho.client.mqttv3.IMqttToken
import org.eclipse.paho.client.mqttv3.MqttAsyncClient
import org.eclipse.paho.client.mqttv3.MqttConnectOptions
import org.eclipse.paho.client.mqttv3.MqttException
import java.io.IOException
import java.net.Socket
import java.util.Locale
import kotlin.concurrent.thread

class MainActivity : AppCompatActivity(), SensorEventListener {
    private lateinit var sensorManager: SensorManager
    private var accelerometer: Sensor? = null
    private var gyroscope: Sensor? = null
    private lateinit var tvData: TextView
    private var mqttClient: MqttAsyncClient? = null
    private lateinit var waveformView: WaveformView
    
    // 传感器数据存储
    private var accX = 0f
    private var accY = 0f
    private var accZ = 0f
    private var gyroX = 0f
    private var gyroY = 0f
    private var gyroZ = 0f
    
    // 音频录制相关
    private var audioRecord: AudioRecord? = null
    private var isRecording = false
    private val sampleRate = 16000
    private val channelConfig = AudioFormat.CHANNEL_IN_MONO
    private val audioFormat = AudioFormat.ENCODING_PCM_16BIT
    private val bufferSize = AudioRecord.getMinBufferSize(sampleRate, channelConfig, audioFormat)
    
    companion object {
        private const val RECORD_AUDIO_PERMISSION_REQUEST = 1001
    }
    
    // MQTT配置参数



    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        tvData = findViewById(R.id.tvSensorData)
        waveformView = findViewById(R.id.waveform)
        // 初始化传感器系统服务
        sensorManager = getSystemService(SENSOR_SERVICE) as SensorManager
        accelerometer = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER)
        gyroscope = sensorManager.getDefaultSensor(Sensor.TYPE_GYROSCOPE)

        // 检查音频录制权限
        checkAudioPermission()

        // 初始化MQTT客户端
        initMqttClient()
    }

    // 后期改成注册模式
    private val mqttServerUri = "tcp://111.229.193.95:1883" // 局域网服务器IP
    private val mqttUsername = "admin"    // 留空如果无需认证
    private val mqttPassword = "20250322"    // 留空如果无需认证
    private val mqttTopic = "sensors"
    private val audioTopic = "audio"
    private fun initMqttClient() {
        try {
            val clientId = "Android_${Build.MODEL}_${System.currentTimeMillis()}"
                .replace(Regex("[^a-zA-Z0-9_]"), "")

            mqttClient = MqttAsyncClient(mqttServerUri, clientId, null).apply {
                val options = MqttConnectOptions().apply {
                    // 认证配置
                    if (mqttUsername.isNotEmpty() && mqttPassword.isNotEmpty()) {
                        userName = mqttUsername
                        password = mqttPassword.toCharArray()
                    }

                    // 连接参数
                    isCleanSession = true
                    isAutomaticReconnect = true
                    maxReconnectDelay = 5000
                    connectionTimeout = 10
                }

                connect(options, null, object : IMqttActionListener {
                    override fun onSuccess(token: IMqttToken?) {
                        Log.i("MQTT", "Connected to LAN server: $mqttServerUri")
                        startSensorAfterConnection()
                    }

                    override fun onFailure(token: IMqttToken?, ex: Throwable?) {
                        Log.e("MQTT", "Connection to LAN failed", ex)
                        showError("无法连接本地服务器")
                    }
                })
            }
        } catch (e: Exception) {
            Log.e("MQTT", "LAN connection setup failed", e)
            showError("服务器配置错误: ${e.message}")
        }
    }

    private fun startSensorAfterConnection() {
        runOnUiThread {
            accelerometer?.let {
                sensorManager.registerListener(this, it, 2500)
            }
            gyroscope?.let {
                sensorManager.registerListener(this, it, 2500)
            }
            // 只有在有录音权限时才开始音频录制
            if (hasAudioPermission()) {
                startAudioRecording()
            }
        }
    }
    
    private fun checkAudioPermission() {
        when {
            ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) 
                == PackageManager.PERMISSION_GRANTED -> {
                Log.d("AUDIO", "Audio permission already granted")
                return
            }
            ActivityCompat.shouldShowRequestPermissionRationale(this, Manifest.permission.RECORD_AUDIO) -> {
                // 用户之前拒绝过权限，显示解释
                showPermissionRationale()
            }
            else -> {
                // 直接请求权限
                ActivityCompat.requestPermissions(
                    this,
                    arrayOf(Manifest.permission.RECORD_AUDIO),
                    RECORD_AUDIO_PERMISSION_REQUEST
                )
            }
        }
    }
    
    private fun showPermissionRationale() {
        runOnUiThread {
            AlertDialog.Builder(this)
                .setTitle("需要录音权限")
                .setMessage("应用需要录音权限来采集音频数据。请允许录音权限以使用完整功能。")
                .setPositiveButton("授权") { _, _ ->
                    ActivityCompat.requestPermissions(
                        this,
                        arrayOf(Manifest.permission.RECORD_AUDIO),
                        RECORD_AUDIO_PERMISSION_REQUEST
                    )
                }
                .setNegativeButton("取消") { dialog, _ ->
                    dialog.dismiss()
                    Toast.makeText(this, "录音功能将无法使用", Toast.LENGTH_LONG).show()
                }
                .show()
        }
    }
    
    private fun hasAudioPermission(): Boolean {
        return ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED
    }
    
    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray
    ) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults)
        when (requestCode) {
            RECORD_AUDIO_PERMISSION_REQUEST -> {
                if (grantResults.isNotEmpty() && grantResults[0] == PackageManager.PERMISSION_GRANTED) {
                    Log.d("AUDIO", "Audio recording permission granted")
                    // 权限被授予后，如果MQTT已连接，开始录音
                    if (mqttClient?.isConnected == true) {
                        startAudioRecording()
                    }
                } else {
                    Log.w("AUDIO", "Audio recording permission denied")
                    Toast.makeText(this, "需要录音权限才能使用音频功能", Toast.LENGTH_LONG).show()
                }
            }
        }
    }
    
    private fun startAudioRecording() {
        // 双重检查权限状态
        if (!hasAudioPermission()) {
            Log.w("AUDIO", "Attempting to record without permission")
            checkAudioPermission() // 重新请求权限
            return
        }
        
        // 如果已经在录音，先停止
        if (isRecording) {
            Log.d("AUDIO", "Already recording, stopping first")
            stopAudioRecording()
        }
        
        try {
            audioRecord = AudioRecord(
                MediaRecorder.AudioSource.MIC,
                sampleRate,
                channelConfig,
                audioFormat,
                bufferSize
            )
            
            if (audioRecord?.state != AudioRecord.STATE_INITIALIZED) {
                Log.e("AUDIO", "AudioRecord initialization failed - state: ${audioRecord?.state}")
                showError("音频录制初始化失败")
                return
            }
            
            audioRecord?.startRecording()
            
            // 检查录制状态
            if (audioRecord?.recordingState != AudioRecord.RECORDSTATE_RECORDING) {
                Log.e("AUDIO", "Failed to start recording - state: ${audioRecord?.recordingState}")
                showError("无法开始录音，请检查权限设置")
                return
            }
            
            isRecording = true
            
            // 在后台线程中录制音频
            thread {
                recordAudio()
            }
            
            Log.d("AUDIO", "Audio recording started successfully")
        } catch (e: SecurityException) {
            Log.e("AUDIO", "Security exception when starting audio recording", e)
            showError("录音权限被拒绝，请在设置中允许录音权限")
        } catch (e: IllegalStateException) {
            Log.e("AUDIO", "Illegal state when starting audio recording", e)
            showError("录音设备被占用或不可用")
        } catch (e: Exception) {
            Log.e("AUDIO", "Failed to start audio recording", e)
            showError("录音启动失败: ${e.message}")
        }
    }
    
    private fun recordAudio() {
        val audioBuffer = ShortArray(bufferSize / 2) // 16-bit PCM
        
        while (isRecording && audioRecord?.recordingState == AudioRecord.RECORDSTATE_RECORDING) {
            val bytesRead = audioRecord?.read(audioBuffer, 0, audioBuffer.size) ?: 0
            
            if (bytesRead > 0) {
                val audioSamples = audioBuffer.copyOf(bytesRead)
                
                // 将音频数据添加到波形视图（不进行下采样）
                runOnUiThread {
                    waveformView.addAudioData(audioSamples)
                }
                
                // 发送音频数据到MQTT
                sendAudioToMqtt(audioSamples)
            }
        }
    }
    
    private fun sendAudioToMqtt(audioData: ShortArray) {
        if (mqttClient?.isConnected != true) {
            return
        }
        
        try {
            // 检查音频数据是否有效
            if (audioData.isEmpty()) {
                Log.w("AUDIO", "Empty audio data, skipping")
                return
            }

            
            // 将音频数据转换为Base64编码
            val byteArray = ByteArray(audioData.size * 2)
            for (i in audioData.indices) {
                val value = audioData[i].toInt()
                byteArray[i * 2] = (value and 0xFF).toByte()
                byteArray[i * 2 + 1] = (value shr 8 and 0xFF).toByte()
            }
            
            var base64Audio = Base64.encodeToString(byteArray, Base64.NO_WRAP)

            // 强制清理所有可能的控制字符
            base64Audio = base64Audio.filter { char ->
                char.code >= 32 || char == ' '
            }
            
            val timestamp = System.currentTimeMillis()
            
            // 使用JSON库构造JSON，更安全
            val jsonObject = JSONObject().apply {
                put("audio_data", base64Audio)
                put("sample_rate", sampleRate)
                put("channels", 1)
                put("format", "pcm_16bit")
                put("samples", audioData.size)
                put("timestamp", timestamp)
            }
            
            val payload = jsonObject.toString()
            
            // 验证最终payload
            val finalPayload = payload.toByteArray(Charsets.UTF_8)
            Log.d("AUDIO", "Sending audio payload size: ${finalPayload.size}, samples: ${audioData.size}")
            
            mqttClient?.publish(
                audioTopic,
                finalPayload,
                0,
                false
            )
            
        } catch (e: Exception) {
            Log.e("AUDIO", "Failed to send audio data", e)
        }
    }
    
    private fun stopAudioRecording() {
        isRecording = false
        audioRecord?.apply {
            if (recordingState == AudioRecord.RECORDSTATE_RECORDING) {
                stop()
            }
            if (state == AudioRecord.STATE_INITIALIZED) {
                release()
            }
        }
        audioRecord = null
        Log.d("AUDIO", "Audio recording stopped")
    }

    private fun showError(message: String) {
        runOnUiThread {
            Toast.makeText(this, message, Toast.LENGTH_LONG).show()
        }
    }

    override fun onResume() {
        super.onResume()
        // 恢复时检查MQTT连接状态
        if (mqttClient?.isConnected != true) {
            initMqttClient()
        }
    }

    override fun onPause() {
        super.onPause()
        // 释放传感器监听
        sensorManager.unregisterListener(this)
        // 停止音频录制
        stopAudioRecording()
        // 断开MQTT连接
        try {
            mqttClient?.disconnect()?.actionCallback = object : IMqttActionListener {
                override fun onSuccess(asyncActionToken: IMqttToken?) {
                    Log.d("MQTT", "Disconnected successfully")
                }

                override fun onFailure(asyncActionToken: IMqttToken?, exception: Throwable?) {
                    Log.e("MQTT", "Disconnect failed", exception)
                }
            }
        } catch (e: Exception) {
            Log.e("MQTT", "Disconnect error", e)
        }
    }

    override fun onSensorChanged(event: SensorEvent) {
        when (event.sensor.type) {
            Sensor.TYPE_ACCELEROMETER -> {
                accX = event.values[0]
                accY = event.values[1]
                accZ = event.values[2]
                
                // 更新UI显示
                val dataText = """
                    加速度计 (m/s²):
                    X: %.2f, Y: %.2f, Z: %.2f
                    
                    陀螺仪 (rad/s):
                    X: %.2f, Y: %.2f, Z: %.2f
                """.trimIndent().format(Locale.US, accX, accY, accZ, gyroX, gyroY, gyroZ)
                tvData.text = dataText

                waveformView.addData(accX, accY, accZ)
                // 发送到MQTT
                sendToMqtt()
            }
            Sensor.TYPE_GYROSCOPE -> {
                gyroX = event.values[0]
                gyroY = event.values[1]
                gyroZ = event.values[2]
                
                // 陀螺仪数据更新时也更新UI
                val dataText = """
                    加速度计 (m/s²):
                    X: %.2f, Y: %.2f, Z: %.2f
                    
                    陀螺仪 (rad/s):
                    X: %.2f, Y: %.2f, Z: %.2f
                """.trimIndent().format(Locale.US, accX, accY, accZ, gyroX, gyroY, gyroZ)
                tvData.text = dataText
            }
        }
    }

    private fun sendToMqtt() {
        if (mqttClient?.isConnected != true) {
            Log.w("MQTT", "Attempted to send while disconnected")
            return
        }

        try {
            val timestamp = System.currentTimeMillis()

            val payload = buildString {
                append("{\n")
                append("\"x\": ${"%.6f".format(Locale.US, accX.toDouble())},\n")
                append("\"y\": ${"%.6f".format(Locale.US, accY.toDouble())},\n")
                append("\"z\": ${"%.6f".format(Locale.US, accZ.toDouble())},\n")
                append("\"gx\": ${"%.6f".format(Locale.US, gyroX.toDouble())},\n")
                append("\"gy\": ${"%.6f".format(Locale.US, gyroY.toDouble())},\n")
                append("\"gz\": ${"%.6f".format(Locale.US, gyroZ.toDouble())},\n")
                append("\"timestamp\": $timestamp\n")
                append("}")
            }

            mqttClient?.publish(
                mqttTopic,
                payload.toByteArray(Charsets.UTF_8),
                0,  // QoS 0
                false
            )
            Log.d("MQTT_PUB", "Published: $payload")
        } catch (e: Exception) {
            Log.e("MQTT", "Publish error", e)
            runOnUiThread {
                Toast.makeText(this, "发送失败: ${e.message}", Toast.LENGTH_SHORT).show()
            }
        }
    }


    override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) {
        // 精度变化处理（可选）
    }
}