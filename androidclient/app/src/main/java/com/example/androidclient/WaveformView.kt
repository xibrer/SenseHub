package com.example.androidclient

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.graphics.Path
import android.util.AttributeSet
import android.view.View

class WaveformView(context: Context, attrs: AttributeSet?) : View(context, attrs) {
    private val maxDataPoints = 2000
    private val dataX = ArrayDeque<Float>()
    private val dataY = ArrayDeque<Float>()
    private val dataZ = ArrayDeque<Float>()
    
    // 音频数据存储，不进行下采样
    private val audioData = ArrayDeque<Short>()
    private val maxAudioDataPoints = 5000 // 增加音频数据点以保持更多细节

    private val paintAxis = Paint().apply {
        color = Color.WHITE
        strokeWidth = 2f
        style = Paint.Style.STROKE
    }

    private val paintX = Paint().apply {
        color = Color.RED
        strokeWidth = 3f
        style = Paint.Style.STROKE
    }

    private val paintY = Paint().apply {
        color = Color.GREEN
        strokeWidth = 3f
        style = Paint.Style.STROKE
    }

    private val paintZ = Paint().apply {
        color = Color.BLUE
        strokeWidth = 3f
        style = Paint.Style.STROKE
    }
    
    // 音频波形绘制画笔
    private val paintAudio = Paint().apply {
        color = Color.rgb(255, 165, 0) // 橘色
        strokeWidth = 2f
        style = Paint.Style.STROKE
    }
    
    // 音频区域分割线画笔
    private val paintDivider = Paint().apply {
        color = Color.GRAY
        strokeWidth = 3f
        style = Paint.Style.STROKE
    }

    private val pathX = Path()
    private val pathY = Path()
    private val pathZ = Path()
    private val pathAudio = Path()

    fun addData(x: Float, y: Float, z: Float) {
        synchronized(this) {
            // 添加新数据并保持队列长度
            dataX.addLast(x)
            if (dataX.size > maxDataPoints) dataX.removeFirst()

            dataY.addLast(y)
            if (dataY.size > maxDataPoints) dataY.removeFirst()

            dataZ.addLast(z)
            if (dataZ.size > maxDataPoints) dataZ.removeFirst()
        }
        postInvalidate()
    }
    
    // 添加音频数据的方法，进行2倍下采样
    fun addAudioData(audioSamples: ShortArray) {
        synchronized(this) {
            // 进行2倍下采样：每两个样本取一个
            for (i in audioSamples.indices step 16) {
                audioData.addLast(audioSamples[i])
                if (audioData.size > maxAudioDataPoints) {
                    audioData.removeFirst()
                }
            }
        }
        postInvalidate()
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)

        // 绘制背景
        canvas.drawColor(Color.WHITE)
        
        // 计算传感器区域和音频区域的高度
        val sensorAreaHeight = height * 0.6f // 传感器数据占60%高度
        val audioAreaHeight = height * 0.4f  // 音频数据占40%高度
        val dividerY = sensorAreaHeight
        
        // 绘制传感器数据区域
        canvas.save()
        canvas.clipRect(0f, 0f, width.toFloat(), sensorAreaHeight)
        drawSensorAxes(canvas, sensorAreaHeight)
        drawSensorWaveform(canvas, dataX, pathX, paintX, sensorAreaHeight)
        drawSensorWaveform(canvas, dataY, pathY, paintY, sensorAreaHeight)
        drawSensorWaveform(canvas, dataZ, pathZ, paintZ, sensorAreaHeight)
        canvas.restore()
        
        // 绘制分割线
        canvas.drawLine(0f, dividerY, width.toFloat(), dividerY, paintDivider)
        
        // 绘制音频数据区域
        canvas.save()
        canvas.clipRect(0f, dividerY, width.toFloat(), height.toFloat())
        drawAudioAxes(canvas, dividerY, audioAreaHeight)
        drawAudioWaveform(canvas, audioData, pathAudio, paintAudio, dividerY, audioAreaHeight)
        canvas.restore()
    }

    private fun drawSensorAxes(canvas: Canvas, areaHeight: Float) {
        val centerY = areaHeight / 2f
        // 水平中线
        canvas.drawLine(0f, centerY, width.toFloat(), centerY, paintAxis)
        // 垂直刻度线
        repeat(5) { i ->
            val x = width * (i + 1) / 6f
            canvas.drawLine(x, centerY - 10f, x, centerY + 10f, paintAxis)
        }
    }
    
    private fun drawAudioAxes(canvas: Canvas, startY: Float, areaHeight: Float) {
        val centerY = startY + areaHeight / 2f
        // 水平中线
        canvas.drawLine(0f, centerY, width.toFloat(), centerY, paintAxis)
        // 垂直刻度线
        repeat(5) { i ->
            val x = width * (i + 1) / 6f
            canvas.drawLine(x, centerY - 5f, x, centerY + 5f, paintAxis)
        }
    }

    private fun drawSensorWaveform(canvas: Canvas, data: Collection<Float>, path: Path, paint: Paint, areaHeight: Float) {
        if (data.isEmpty()) return

        path.reset()
        val dx = width.toFloat() / (maxDataPoints - 1)
        val centerY = areaHeight / 2f
        val scale = areaHeight / 40f  // 假设数据范围在-20到+20之间

        data.forEachIndexed { index, value ->
            val x = index * dx
            val y = centerY - (value * scale)

            if (index == 0) {
                path.moveTo(x, y)
            } else {
                path.lineTo(x, y)
            }
        }
        canvas.drawPath(path, paint)
    }
    
    private fun drawAudioWaveform(canvas: Canvas, data: Collection<Short>, path: Path, paint: Paint, startY: Float, areaHeight: Float) {
        if (data.isEmpty()) return

        path.reset()
        val dx = width.toFloat() / maxAudioDataPoints
        val centerY = startY + areaHeight / 2f
        val scale = areaHeight / 65536f  // 16位音频数据范围是-32768到32767

        data.forEachIndexed { index, value ->
            val x = index * dx
            val y = centerY - (value * scale)

            if (index == 0) {
                path.moveTo(x, y)
            } else {
                path.lineTo(x, y)
            }
        }
        canvas.drawPath(path, paint)
    }
}