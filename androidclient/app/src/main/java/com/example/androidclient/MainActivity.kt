package com.example.androidclient


import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.widget.Button
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
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
    // MQTT配置参数



    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)
        tvData = findViewById(R.id.tvSensorData)
        waveformView = findViewById(R.id.waveform)
        // 初始化传感器系统服务
        sensorManager = getSystemService(SENSOR_SERVICE) as SensorManager
        accelerometer = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER)

        // 初始化MQTT客户端
        initMqttClient()
    }

    // 后期改成注册模式
    private val mqttServerUri = "tcp://111.229.193.95:1883" // 局域网服务器IP
    private val mqttUsername = "admin"    // 留空如果无需认证
    private val mqttPassword = "20250322"    // 留空如果无需认证
    private val mqttTopic = "sensors"
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
                sensorManager.registerListener(this, it, 25)
            }
        }
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

            // 更新UI显示
            val dataText = """
                X: %.2f m/s²
                Y: %.2f m/s²
                Z: %.2f m/s²
            """.trimIndent().format(Locale.US, x, y, z)
            tvData.text = dataText

            waveformView.addData(x, y, z)
            // 发送到MQTT
            sendToMqtt(x, y, z)
        }
    }

    private fun sendToMqtt(x: Float, y: Float, z: Float) {
        if (mqttClient?.isConnected != true) {
            Log.w("MQTT", "Attempted to send while disconnected")
            return
        }

        try {
            val timestamp = System.currentTimeMillis()

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