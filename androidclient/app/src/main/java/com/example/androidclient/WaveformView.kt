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

    private val pathX = Path()
    private val pathY = Path()
    private val pathZ = Path()

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

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)

        // 绘制背景和坐标轴
        canvas.drawColor(Color.WHITE)
        drawAxes(canvas)

        // 绘制波形
        drawWaveform(canvas, dataX, pathX, paintX)
        drawWaveform(canvas, dataY, pathY, paintY)
        drawWaveform(canvas, dataZ, pathZ, paintZ)
    }

    private fun drawAxes(canvas: Canvas) {
        val centerY = height / 2f
        // 水平中线
        canvas.drawLine(0f, centerY, width.toFloat(), centerY, paintAxis)
        // 垂直刻度线
        repeat(5) { i ->
            val x = width * (i + 1) / 6f
            canvas.drawLine(x, centerY - 10f, x, centerY + 10f, paintAxis)
        }
    }

    private fun drawWaveform(canvas: Canvas, data: Collection<Float>, path: Path, paint: Paint) {
        if (data.isEmpty()) return

        path.reset()
        val dx = width.toFloat() / (maxDataPoints - 1)
        val centerY = height / 2f
        val scale = height / 40f  // 假设数据范围在-20到+20之间

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