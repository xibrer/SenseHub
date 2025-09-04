package com.example.androidclient


import android.Manifest
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
    private lateinit var tvData: TextView
    private var mqttClient: MqttAsyncClient? = null
    private lateinit var waveformView: WaveformView
    
    // 控制按钮
    private lateinit var btnToggle: Button
    
    // 暂停控制状态（合并为一个状态）
    private var isPaused = false
    
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
        btnToggle = findViewById(R.id.btnToggle)
        
        // 初始化传感器系统服务
        sensorManager = getSystemService(SENSOR_SERVICE) as SensorManager
        accelerometer = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER)
        
        // 设置按钮点击事件
        setupButtonListeners()

        // 检查音频录制权限
        checkAudioPermission()

        // 初始化MQTT客户端
        initMqttClient()
    }
    
    private fun setupButtonListeners() {
        // 统一的暂停/恢复按钮
        btnToggle.setOnClickListener {
            isPaused = !isPaused
            btnToggle.text = if (isPaused) "恢复" else "暂停"
            waveformView.setDrawingPaused(isPaused)
            
            if (isPaused) {
                Toast.makeText(this, "已暂停数据发送和波形绘制", Toast.LENGTH_SHORT).show()
            } else {
                Toast.makeText(this, "已恢复数据发送和波形绘制", Toast.LENGTH_SHORT).show()
            }
        }
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
            // 开始音频录制
            startAudioRecording()
        }
    }
    
    private fun checkAudioPermission() {
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) 
            != PackageManager.PERMISSION_GRANTED) {
            ActivityCompat.requestPermissions(
                this,
                arrayOf(Manifest.permission.RECORD_AUDIO),
                RECORD_AUDIO_PERMISSION_REQUEST
            )
        }
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
                } else {
                    Toast.makeText(this, "需要录音权限才能使用音频功能", Toast.LENGTH_LONG).show()
                }
            }
        }
    }
    
    private fun startAudioRecording() {
        if (ContextCompat.checkSelfPermission(this, Manifest.permission.RECORD_AUDIO) 
            != PackageManager.PERMISSION_GRANTED) {
            return
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
                Log.e("AUDIO", "AudioRecord initialization failed")
                return
            }
            
            audioRecord?.startRecording()
            isRecording = true
            
            // 在后台线程中录制音频
            thread {
                recordAudio()
            }
            
            Log.d("AUDIO", "Audio recording started")
        } catch (e: Exception) {
            Log.e("AUDIO", "Failed to start audio recording", e)
        }
    }
    
    private fun recordAudio() {
        val audioBuffer = ShortArray(bufferSize / 2) // 16-bit PCM
        
        while (isRecording && audioRecord?.recordingState == AudioRecord.RECORDSTATE_RECORDING) {
            val recordTimestamp = System.currentTimeMillis()
            val bytesRead = audioRecord?.read(audioBuffer, 0, audioBuffer.size) ?: 0
            
            if (bytesRead > 0) {
                val audioSamples = audioBuffer.copyOf(bytesRead)
                // 在音频数据记录后立即添加时间戳

                
                // 将音频数据添加到波形视图（不进行下采样）
                runOnUiThread {
                    if (!isPaused) {
                        waveformView.addAudioData(audioSamples)
                    }
                }
                
                // 发送音频数据到MQTT，传入记录时的时间戳
                if (!isPaused) {
                    sendAudioToMqtt(audioSamples, recordTimestamp)
                }
            }
        }
    }
    
    private fun sendAudioToMqtt(audioData: ShortArray, timestamp: Long) {
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
            
            // 使用传入的记录时间戳，而不是在此处生成新的时间戳
            
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
        if (event.sensor.type == Sensor.TYPE_ACCELEROMETER) {
            val x = event.values[0]
            val y = event.values[1]
            val z = event.values[2]
            // 在传感器数据接收后立即添加时间戳
            val sensorTimestamp = System.currentTimeMillis()

            // 更新UI显示
            val dataText = """
                X: %.2f m/s²
                Y: %.2f m/s²
                Z: %.2f m/s²
            """.trimIndent().format(Locale.US, x, y, z)
            tvData.text = dataText

            // 只有在未暂停时才添加数据到波形视图和发送数据
            if (!isPaused) {
                waveformView.addData(x, y, z)
                sendToMqtt(x, y, z, sensorTimestamp)
            }
        }
    }

    private fun sendToMqtt(x: Float, y: Float, z: Float, timestamp: Long) {
        if (mqttClient?.isConnected != true) {
            Log.w("MQTT", "Attempted to send while disconnected")
            return
        }

        try {
            // 使用传入的传感器数据接收时间戳，而不是在此处生成新的时间戳

            val payload = buildString {
                append("{\n")
                append("\"x\": ${"%.6f".format(Locale.US, x.toDouble())},\n")
                append("\"y\": ${"%.6f".format(Locale.US, y.toDouble())},\n")
                append("\"z\": ${"%.6f".format(Locale.US, z.toDouble())},\n")
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