import { theme } from "./theme.js";

export interface BarChartData {
  label: string;
  value: number;
}

export interface BarChartProps {
  title: string;
  data: BarChartData[];
  watermark?: string;
}

export function BarChart({
  title,
  data,
  watermark = "InviteAnalytics",
}: BarChartProps) {
  const maxValue = Math.max(...data.map((d) => d.value), 1);
  const barHeight = 36;
  const barGap = 12;
  const labelWidth = 200;
  const chartPadding = { top: 70, right: 60, bottom: 50, left: 20 };
  const _chartWidth =
    theme.width - chartPadding.left - chartPadding.right - labelWidth;

  // Generate nice grid lines
  const gridStep = Math.max(1, Math.ceil(maxValue / 5));
  const gridLines: number[] = [];
  for (let i = 0; i <= maxValue; i += gridStep) {
    gridLines.push(i);
  }
  // Always include the max
  if (gridLines[gridLines.length - 1] < maxValue) {
    gridLines.push(maxValue + gridStep);
  }
  const gridMax = gridLines[gridLines.length - 1];

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: theme.width,
        height: theme.height,
        backgroundColor: theme.background,
        padding: `${chartPadding.top}px ${chartPadding.right}px ${chartPadding.bottom}px ${chartPadding.left}px`,
        fontFamily: "Inter, sans-serif",
      }}
    >
      {/* Title */}
      <div
        style={{
          display: "flex",
          fontSize: 18,
          fontWeight: 700,
          color: theme.textPrimary,
          marginBottom: 24,
          textAlign: "center",
          justifyContent: "center",
        }}
      >
        {title}
      </div>

      {/* Chart area */}
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          flex: 1,
          position: "relative",
        }}
      >
        {/* Bars */}
        {data.map((item, i) => (
          <div
            key={i}
            style={{
              display: "flex",
              alignItems: "center",
              height: barHeight,
              marginBottom: barGap,
            }}
          >
            {/* Label */}
            <div
              style={{
                display: "flex",
                width: labelWidth,
                fontSize: 13,
                color: theme.textSecondary,
                justifyContent: "flex-end",
                paddingRight: 12,
                overflow: "hidden",
              }}
            >
              {item.label}
            </div>

            {/* Bar */}
            <div
              style={{
                display: "flex",
                position: "relative",
                flex: 1,
                height: "100%",
                alignItems: "center",
              }}
            >
              <div
                style={{
                  display: "flex",
                  height: barHeight - 4,
                  width: `${(item.value / gridMax) * 100}%`,
                  backgroundColor: theme.barColor,
                  borderRadius: 4,
                  minWidth: item.value > 0 ? 4 : 0,
                }}
              />
              {/* Value label */}
              <div
                style={{
                  display: "flex",
                  fontSize: 12,
                  color: theme.textSecondary,
                  paddingLeft: 8,
                }}
              >
                {item.value}
              </div>
            </div>
          </div>
        ))}

        {/* X-axis labels */}
        <div
          style={{
            display: "flex",
            marginLeft: labelWidth,
            marginTop: 8,
            justifyContent: "space-between",
          }}
        >
          {gridLines.map((v, i) => (
            <div
              key={i}
              style={{
                display: "flex",
                fontSize: 11,
                color: theme.textSecondary,
              }}
            >
              {v}
            </div>
          ))}
        </div>

        {/* X-axis label */}
        <div
          style={{
            display: "flex",
            marginLeft: labelWidth,
            marginTop: 4,
            justifyContent: "center",
            fontSize: 12,
            color: theme.textSecondary,
          }}
        >
          Users Invited
        </div>
      </div>

      {/* Watermark */}
      <div
        style={{
          display: "flex",
          position: "absolute",
          bottom: 16,
          right: 20,
          fontSize: 11,
          color: theme.watermarkColor,
        }}
      >
        {watermark}
      </div>
    </div>
  );
}
