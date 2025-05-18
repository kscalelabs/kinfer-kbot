import pandas as pd
import re
import matplotlib.pyplot as plt
import os
import numpy as np
import shutil
import argparse

# Timestamp pattern (ISO format)
TIMESTAMP_REGEX = re.compile(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)")

# Print type pattern (after timestamp and spaces)
PRINT_TYPE_REGEX = re.compile(r"Z\s+([^\s]+)")

# Thread ID pattern
THREAD_ID_REGEX = re.compile(r"ThreadId\((\d+)\)")


# EVENT_REGEX = re.compile(r"((?:(?!::(?:START|END)).)+?)::(START|END)")
# Event pattern (match everything after ThreadId including START/END if present)
EVENT_REGEX = re.compile(r"ThreadId\(\d+\)\s+kinfer_kbot::[^:]+: src/[^:]+:\d+: ([^\s,]+)")

# UUID pattern
UUID_REGEX = re.compile(r"uuid=([0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12})")

# Elapsed time pattern
ELAPSED_REGEX = re.compile(r"elapsed: Ok\(([^)]+)\)")

def log_to_dataframe(log_file):
    print(f"Parsing log file: {log_file}")
    
    # Store parsed data in lists
    data = []
    
    # Open and read the log file
    with open(log_file, 'r', encoding='utf-8', errors='ignore') as f:
        for line in f:
            timestamp_match = TIMESTAMP_REGEX.search(line)
            event_match = EVENT_REGEX.search(line)
            print_type_match = PRINT_TYPE_REGEX.search(line)
            thread_id_match = THREAD_ID_REGEX.search(line)
            uuid_match = UUID_REGEX.search(line)
            elapsed_match = ELAPSED_REGEX.search(line)

            timestamp = timestamp_match.group(1) if timestamp_match else None
            event = event_match.group(1) if event_match else None
            print_type = print_type_match.group(1) if print_type_match else None
            thread_id = thread_id_match.group(0) if thread_id_match else None
            uuid = uuid_match.group(1) if uuid_match else None
            elapsed = elapsed_match.group(1) if elapsed_match else None
            
            
            if print_type == "DEBUG":
                data.append({
                    "timestamp": timestamp,
                    "event": event,
                    "print_type": print_type,
                    "thread_id": thread_id,
                    "uuid": uuid,
                    "elapsed": elapsed
                })
                

    # Create DataFrame
    df = pd.DataFrame(data)
    print(f"Parsed {len(df)} log entries")
    
    return df

def df_by_type(df):
    # Extract command type from event field (after "provider::" or "runtime::" and before the second "::")
    command_type_regex = re.compile(r'(provider|runtime)::([^:]+)::')
    
    # Create a new column for command type

    breakpoint() 
    df['command_type'] = df['event'].apply(
        lambda x: f"{command_type_regex.search(x).group(1)}::{command_type_regex.search(x).group(2)}::" 
        if command_type_regex.search(x) else None
    )

    breakpoint()

    
    # Convert timestamp to datetime
    df['timestamp'] = pd.to_datetime(df['timestamp'])
    
    # Group by command_type to find consecutive instances
    results = []
    
    # Group by command_type
    grouped = df.groupby('command_type')
    
    for command_type, group in grouped:
        # Sort events by timestamp to ensure proper order
        group = group.sort_values('timestamp')
        
        # Process events in pairs (1-2, 3-4, 5-6, etc.)
        paired_data = []
        
        # Iterate through events in pairs
        for i in range(0, len(group) - 1, 2):
            if i + 1 < len(group):
                first_event = group.iloc[i]
                second_event = group.iloc[i + 1]
                
                # Calculate elapsed time
                elapsed_time = (second_event['timestamp'] - first_event['timestamp']).total_seconds()
                
                # Pass on the erronously large or small events
                if elapsed_time > 0.1 or elapsed_time < 0.0001:
                    continue

                paired_data.append({
                    'command_type': command_type,
                    'first_uuid': first_event['uuid'],
                    'second_uuid': second_event['uuid'],
                    'first_timestamp': first_event['timestamp'],
                    'second_timestamp': second_event['timestamp'],
                    'elapsed_seconds': elapsed_time,
                    'logged_elapsed': second_event['elapsed']
                })
        
        if paired_data:
            # Create DataFrame for this command type
            command_df = pd.DataFrame(paired_data)
            results.append(command_df)
    
    # Combine all results
    if results:
        return pd.concat(results, ignore_index=True)
    else:
        return pd.DataFrame()

def plot_histograms(df, output_dir, custom_title):
    command_types = df['command_type'].unique()
    
    for command in command_types:
        cmd_data = df[df['command_type'] == command]
        if len(cmd_data) < 2:  # Skip if too few data points
            continue
            
        # Convert to milliseconds
        times = cmd_data['elapsed_seconds'].values * 1000
        
        # Create figure with single plot
        plt.figure(figsize=(15, 7))
        
        # Calculate max time and ensure x-axis is at least 0 to 1ms
        # max_time = np.max(times)
        # x_limit = max(1.0, max_time * 1.1)  # Ensure at least 0-1ms range
        
        # Create bins from 0 to max value
        # num_bins = min(200, max(91, int(x_limit*10)+1))  # Reasonable number of bins
        # bins = np.linspace(0, x_limit, num_bins)
        x_limit = 25

        bin_size = 0.1
        bins = np.arange(0, 25 + bin_size, bin_size)
        
        
        # Plot histogram
        plt.hist(times, bins=bins, color='skyblue', edgecolor='black')
        plt.yscale('log')  # Keep y-axis as log scale
        plt.ylabel('Frequency (Log Scale)')
        
        # Set x-axis limit
        # Artifically changing limit to make comparison easy. Does not effect metrics calculated 
        plt.xlim(0, x_limit)
        
        # Format x-axis ticks to show 2 decimal places
        plt.gca().xaxis.set_major_formatter(plt.FuncFormatter(lambda x, _: f'{x:.2f}'))
            
        plt.title(f'{custom_title} - Histogram of Event Time Deltas (ms)\n({command})', wrap=True)
        plt.xlabel('Time Elapsed (milliseconds)')
        plt.grid(axis='y', alpha=0.75, which='both')
        
        # Add statistics with 4 decimal places
        if len(times) > 0:
            mean_val = np.mean(times)
            median_val = np.median(times)
            variance_val = np.var(times)
            std_val = np.std(times)
            plt.axvline(mean_val, color='red', linestyle='dashed', linewidth=1, 
                       label=f'Mean: {mean_val:.4f}ms')
            plt.axvline(median_val, color='green', linestyle='dashed', linewidth=1, 
                       label=f'Median: {median_val:.4f}ms')
            plt.text(0.95, 0.95, f"Var: {variance_val:.4f}, Std: {std_val:.4f} \n Bin Size: {bin_size:.4f}. \n Plot truncated at 25ms, but metrics includes.",
            transform=plt.gca().transAxes,
            verticalalignment='top', horizontalalignment='right',
            fontsize=9, bbox=dict(boxstyle="round", facecolor="white", alpha=0.7))

            
            plt.legend()
        
        plt.tight_layout()
        safe_command = command.replace('::', '_').replace('/', '_')
        plt.savefig(os.path.join(output_dir, f"{safe_command}_histogram.png"))
        plt.close()

def plot_detailed(df, output_dir, custom_title):
    """
    Create four plots per command type:
      • histogram (all data)                ┐ share the same x-axis
      • box-plot   (all data)               │ based on IQR-filtered range
      • histogram (outliers removed)        ├── x-axis truncated to ±2 × IQR
      • box-plot   (outliers removed)       ┘
    The function writes the images below  <output_dir>/detailed/…
    """
    import os
    import numpy as np
    import matplotlib.pyplot as plt

    # ------------------------------------------------------------------ #
    #  Directory scaffold                                                #
    # ------------------------------------------------------------------ #
    droot     = os.path.join(output_dir, "detailed")
    d_hist    = os.path.join(droot, "histograms")
    d_box     = os.path.join(droot, "boxplots")
    d_hist_no = os.path.join(droot, "no_outliers_histograms")
    d_box_no  = os.path.join(droot, "no_outliers_boxplots")
    for d in (droot, d_hist, d_box, d_hist_no, d_box_no):
        os.makedirs(d, exist_ok=True)

    # ------------------------------------------------------------------ #
    #  One loop per command type                                         #
    # ------------------------------------------------------------------ #
    for command in df["command_type"].unique():
        cmd_df    = df[df["command_type"] == command]
        times_ms  = cmd_df["elapsed_seconds"].values * 1000        # → milliseconds
        if len(times_ms) == 0:
            continue

        # -------------------- robust range for "raw" plots --------------------
        q1, q3 = np.percentile(times_ms, [25, 75])
        iqr    = q3 - q1
        lo_raw = max(0, q1 - 1.5 * iqr)
        hi_raw = q3 + 1.5 * iqr
        axis_ts = times_ms[(times_ms >= lo_raw) & (times_ms <= hi_raw)]
        if len(axis_ts) == 0:                           # all data were outliers
            axis_ts = times_ms
        xmax_raw = np.max(axis_ts) * 1.10               # +10 %

        # Freedman–Diaconis bin width for raw plots (clamped 0.05–1 ms)
        q25_r, q75_r = np.percentile(axis_ts, [25, 75])
        iqr_r        = q75_r - q25_r
        bw_raw = 2 * iqr_r * (len(axis_ts) ** (-1/3)) if len(axis_ts) > 1 and iqr_r > 0 else 0.1
        bw_raw = np.clip(bw_raw, 0.05, 1.0)
        bins_raw = np.arange(0, xmax_raw + bw_raw, bw_raw)

        safe_cmd = command.replace("::", "_").replace("/", "_")

        # ----------------------------- RAW HISTOGRAM -------------------------
        plt.figure(figsize=(15, 7))
        plt.hist(times_ms, bins=bins_raw, color="skyblue", edgecolor="black")
        plt.yscale("log")
        plt.xlim(0, xmax_raw)
        plt.gca().xaxis.set_major_formatter(plt.FuncFormatter(lambda x, _: f"{x:.2f}"))
        mean_r, med_r, var_r, std_r = np.mean(times_ms), np.median(times_ms), np.var(times_ms), np.std(times_ms)
        plt.axvline(mean_r, color="red",   ls="--", lw=1, label=f"Mean {mean_r:.4f} ms")
        plt.axvline(med_r,  color="green", ls="--", lw=1, label=f"Median {med_r:.4f} ms")
        plt.title(f"{custom_title} – Histogram of Event Time Deltas (ms)\n({command})", wrap=True)
        plt.xlabel("Time Elapsed (milliseconds)")
        plt.ylabel("Frequency (log scale)")
        plt.grid(axis="y", alpha=0.75, which="both")
        plt.text(0.95, 0.95,
                 f"Var {var_r:.4f},  Std {std_r:.4f}\n"
                 f"Bin {bw_raw:.4f} ms\nx-axis based on 1.5×IQR-filtered data",
                 transform=plt.gca().transAxes, ha="right", va="top",
                 fontsize=9, bbox=dict(boxstyle="round", fc="white", alpha=0.7))
        plt.legend()
        plt.tight_layout()
        plt.savefig(os.path.join(d_hist, f"{safe_cmd}_histogram.png"))
        plt.close()

        # ------------------------------ RAW BOX PLOT --------------------------
        plt.figure(figsize=(15, 7))
        plt.boxplot(times_ms, vert=False, patch_artist=True, showmeans=True, meanline=True,
                    boxprops=dict(fc="skyblue", alpha=0.7, lw=1.5, ec="black"),
                    medianprops=dict(color="red",   lw=2),
                    meanprops  =dict(color="green", ls="--", lw=2),
                    whiskerprops=dict(color="black", lw=1.5),
                    capprops    =dict(color="black", lw=1.5),
                    flierprops  =dict(marker="o", mfc="red", mec="black", ms=5, alpha=0.6))
        plt.xlim(0, xmax_raw)
        plt.xlabel("Time Elapsed (milliseconds)")
        plt.title(f"{custom_title} – Box Plot of Event Time Deltas (ms)\n({command})", wrap=True)
        plt.grid(axis="x", alpha=0.3)
        plt.text(0.95, 0.95,
                 f"n={len(times_ms)}\nMin {np.min(times_ms):.4f} ms\n"
                 f"Q1  {q1:.4f} ms\nMedian {med_r:.4f} ms\nMean {mean_r:.4f} ms\n"
                 f"Q3  {q3:.4f} ms\nMax {np.max(times_ms):.4f} ms\nStd {std_r:.4f} ms",
                 transform=plt.gca().transAxes, ha="right", va="top",
                 fontsize=10, bbox=dict(boxstyle="round", fc="white", alpha=0.7))
        plt.tight_layout()
        plt.savefig(os.path.join(d_box, f"{safe_cmd}_boxplot.png"))
        plt.close()

        # ------------------- IQR filtering for "no-outlier" plots -------------
        lo_no = max(0, q1 - 1.5 * iqr)                 # classic 1.5×IQR filter
        hi_no = q3 + 1.5 * iqr
        filt   = times_ms[(times_ms >= lo_no) & (times_ms <= hi_no)]
        if len(filt) == 0:
            continue

        # 2 × IQR axis for the "no-outlier" plots
        lo_axis = max(0, q1 - 2 * iqr)
        hi_axis = q3 + 2 * iqr
        bw_no = 2 * iqr * (len(filt) ** (-1/3)) if len(filt) > 1 and iqr > 0 else 0.1
        bw_no = np.clip(bw_no, 0.05, 1.0)
        bins_no = np.arange(lo_axis, hi_axis + bw_no, bw_no)

        # ------------------- HISTOGRAM (NO OUTLIERS) --------------------------
        plt.figure(figsize=(15, 7))
        plt.hist(filt, bins=bins_no, color="skyblue", edgecolor="black")
        plt.yscale("log")
        plt.xlim(lo_axis, hi_axis)
        plt.gca().xaxis.set_major_formatter(plt.FuncFormatter(lambda x, _: f"{x:.2f}"))
        mean_n, med_n, var_n, std_n = np.mean(filt), np.median(filt), np.var(filt), np.std(filt)
        removed = len(times_ms) - len(filt)
        plt.axvline(mean_n, color="red",   ls="--", lw=1, label=f"Mean {mean_n:.4f} ms")
        plt.axvline(med_n,  color="green", ls="--", lw=1, label=f"Median {med_n:.4f} ms")
        plt.title(f"{custom_title} – Histogram of Event Time Deltas (no outliers)\n({command})", wrap=True)
        plt.xlabel("Time Elapsed (milliseconds)")
        plt.ylabel("Frequency (log scale)")
        plt.grid(axis="y", alpha=0.75, which="both")
        plt.text(0.95, 0.95,
                 f"Var {var_n:.4f},  Std {std_n:.4f}\n"
                 f"Bin {bw_no:.4f} ms\nRemoved {removed} outlier{'s'*(removed!=1)}\n"
                 f"x-axis = Q1–2×IQR … Q3+2×IQR",
                 transform=plt.gca().transAxes, ha="right", va="top",
                 fontsize=9, bbox=dict(boxstyle="round", fc="white", alpha=0.7))
        plt.legend()
        plt.tight_layout()
        plt.savefig(os.path.join(d_hist_no, f"{safe_cmd}_histogram_no_outliers.png"))
        plt.close()

        # ---------------------- BOX PLOT (NO OUTLIERS) ------------------------
        plt.figure(figsize=(15, 7))
        plt.boxplot(filt, vert=False, patch_artist=True, showmeans=True, meanline=True,
                    boxprops=dict(fc="skyblue", alpha=0.7, lw=1.5, ec="black"),
                    medianprops=dict(color="red",   lw=2),
                    meanprops  =dict(color="green", ls="--", lw=2),
                    whiskerprops=dict(color="black", lw=1.5),
                    capprops    =dict(color="black", lw=1.5))
        plt.xlim(lo_axis, hi_axis)
        plt.xlabel("Time Elapsed (milliseconds)")
        plt.title(f"{custom_title} – Box Plot of Event Time Deltas (no outliers)\n({command})", wrap=True)
        plt.grid(axis="x", alpha=0.3)
        plt.text(0.95, 0.95,
                 f"n={len(filt)} (removed {removed})\nMin {np.min(filt):.4f} ms\n"
                 f"Q1  {np.percentile(filt,25):.4f} ms\nMedian {med_n:.4f} ms\n"
                 f"Mean {mean_n:.4f} ms\nQ3  {np.percentile(filt,75):.4f} ms\n"
                 f"Max {np.max(filt):.4f} ms\nStd {std_n:.4f} ms",
                 transform=plt.gca().transAxes, ha="right", va="top",
                 fontsize=10, bbox=dict(boxstyle="round", fc="white", alpha=0.7))
        plt.tight_layout()
        plt.savefig(os.path.join(d_box_no, f"{safe_cmd}_boxplot_no_outliers.png"))
        plt.close()


def plot_command_summary(df, output_dir, custom_title):
    command_types = df['command_type'].unique()
    
    # Group by command type and calculate statistics
    stats = {}
    for cmd in command_types:
        cmd_data = df[df['command_type'] == cmd]
        if len(cmd_data) > 0:
            times = cmd_data['elapsed_seconds'].values * 1000  # Convert to milliseconds
            stats[cmd] = {
                'mean': np.mean(times),
                'median': np.median(times),
                'max': np.max(times),
                'count': len(times)
            }
    
    # Sort commands by median time
    sorted_commands = sorted(stats.keys(), key=lambda x: stats[x]['median'], reverse=True)
    
    # Prepare data for bar chart
    cmd_labels = [cmd for cmd in sorted_commands]
    median_vals = [stats[cmd]['median'] for cmd in sorted_commands]
    mean_vals = [stats[cmd]['mean'] for cmd in sorted_commands]
    max_vals = [stats[cmd]['max'] for cmd in sorted_commands]
    
    # Plot the data
    x = np.arange(len(cmd_labels))
    width = 0.25
    
    fig, ax = plt.subplots(figsize=(16, 10))  # Increased height for better spacing
    rects1 = ax.bar(x - width, median_vals, width, label='Median', color='green', alpha=0.7)
    rects2 = ax.bar(x, mean_vals, width, label='Mean', color='blue', alpha=0.7)
    
    # Add labels and title
    # ax.set_xlabel('Command Type', fontsize=12)
    ax.set_ylabel('Time (milliseconds)', fontsize=12)
    ax.set_title(f'{custom_title} - Command Execution Times by Type', fontsize=14)
    ax.set_xticks(x)
    
    # Create wrapped labels with more meaningful formatting
    wrapped_labels = []
    for cmd in cmd_labels:
        # Split the command into parts and get the last meaningful part
        parts = cmd.split('::')
        # Create wrapped text with newlines for better readability
        if len(parts) > 1:
            # Format with path breaks for better readability
            wrapped_label = '\n'.join(parts)
        else:
            wrapped_label = cmd
        wrapped_labels.append(wrapped_label)
    
    # Set wrapped labels and adjust their properties
    ax.set_xticklabels(wrapped_labels, rotation=0, ha='center', fontsize=10)
    
    # Add spacing between x-axis and labels
    plt.subplots_adjust(bottom=0.3)  # Increase bottom margin for labels
    
    # Move statistics text below the x-axis instead of above the bars
    for i, cmd in enumerate(sorted_commands):
        ax.text(i, -max(max_vals) * 0.15,  # Negative y value to place below axis
                f"n={stats[cmd]['count']}\nMean (ms): {stats[cmd]['mean']:.4f}\nMedian (ms): {stats[cmd]['median']:.4f}",
                ha='center', va='top', rotation=0, size=9)
    
    ax.legend(fontsize=11)
    
    # Add grid and adjust layout
    ax.grid(axis='y', linestyle='--', alpha=0.7)
    
    # Save the plot
    plt.savefig(os.path.join(output_dir, "command_timing_summary.png"))
    plt.close()


def plot_combined_histograms(df, output_dir, custom_title):
    command_types = df['command_type'].unique()
    
    # Create a figure for the combined histogram
    plt.figure(figsize=(15, 7))
    
    # Convert all times to milliseconds
    all_times = df['elapsed_seconds'].values * 1000
    
    # Calculate max time and ensure x-axis is at least 0 to 1ms
    max_time = np.max(all_times)
    x_limit = max(1.0, max_time * 1.1)  # Ensure at least 0-1ms range
    
    # Create bins from 0 to max value
    num_bins = min(200, max(91, int(x_limit*10)+1))  # Reasonable number of bins
    bins = np.linspace(0, x_limit, num_bins)
    
    # Plot histograms for each command type
    for command in command_types:
        cmd_data = df[df['command_type'] == command]
        if len(cmd_data) < 2:  # Skip if too few data points
            continue
        
        # Convert to milliseconds
        times = cmd_data['elapsed_seconds'].values * 1000
        plt.hist(times, bins=bins, alpha=0.3, label=command)
    
    plt.yscale('log')  # Use log scale for y-axis
    plt.xlim(0, x_limit)
    plt.title(f'{custom_title} - Combined Histogram of Event Time Deltas (ms)')
    plt.xlabel('Time Elapsed (milliseconds)')
    plt.ylabel('Frequency (Log Scale)')
    plt.grid(axis='y', alpha=0.75, which='both')
    plt.legend(loc='upper right', bbox_to_anchor=(0.95, 1))
    
    plt.tight_layout()
    plt.savefig(os.path.join(output_dir, "combined_histogram.png"))
    plt.close()

def plot_combined_boxplot(df, output_dir, custom_title):
    command_types = df['command_type'].unique()
    
    # Filter out command types with too few data points
    valid_commands = []
    command_data = []
    
    for command in command_types:
        cmd_data = df[df['command_type'] == command]
        if len(cmd_data) >= 2:
            times = cmd_data['elapsed_seconds'].values * 1000  # Convert to milliseconds
            valid_commands.append(command)
            command_data.append(times)
    
    if not valid_commands:
        return  # No valid commands to plot
    
    # Create figure for combined box plot
    plt.figure(figsize=(15, 10))
    
    # Create box plot
    boxplot = plt.boxplot(command_data, vert=True, patch_artist=True, 
                          labels=valid_commands, showmeans=True, meanline=True)
    
    # Customize box plot appearance
    for box in boxplot['boxes']:
        box.set(color='black', linewidth=1.5)
        box.set(facecolor='skyblue', alpha=0.7)
    for whisker in boxplot['whiskers']:
        whisker.set(color='black', linewidth=1.5)
    for cap in boxplot['caps']:
        cap.set(color='black', linewidth=1.5)
    for median in boxplot['medians']:
        median.set(color='red', linewidth=2)
    for mean in boxplot['means']:
        mean.set(color='green', linestyle='--', linewidth=2)
    for flier in boxplot['fliers']:
        flier.set(marker='o', markerfacecolor='red', markeredgecolor='black', markersize=5, alpha=0.6)
    
    # Calculate overall statistics for the legend
    plt.axhline(0, color='black', linewidth=0.5)  # Add a line at y=0
    
    # Add legend for median and mean lines
    plt.plot([], [], color='red', linewidth=2, label='Median')
    plt.plot([], [], color='green', linestyle='--', linewidth=2, label='Mean')
    plt.legend(loc='upper right')
    
    # Format x-axis labels for better readability
    wrapped_labels = []
    for cmd in valid_commands:
        parts = cmd.split('::')
        wrapped_label = '\n'.join(parts)
        wrapped_labels.append(wrapped_label)
    
    plt.xticks(range(1, len(valid_commands) + 1), wrapped_labels, rotation=45, ha='right')
    
    # Add title and labels
    plt.title(f'{custom_title} - Comparative Box Plot of Command Execution Times', fontsize=14)
    plt.ylabel('Time Elapsed (milliseconds)', fontsize=12)
    plt.grid(axis='y', alpha=0.3)
    
    # Ensure y-axis starts at 0 but shows all data
    plt.ylim(bottom=0)
    
    # Add annotation with statistical table
    stats_text = "Command Type Statistics:\n"
    stats_text += "Command | n | Mean (ms) | Median (ms) | Std Dev (ms)\n"
    stats_text += "--------|---|-----------|-------------|-------------\n"
    
    for i, (cmd, times) in enumerate(zip(valid_commands, command_data)):
        mean_val = np.mean(times)
        median_val = np.median(times)
        std_val = np.std(times)
        cmd_short = cmd.split('::')[-1] if '::' in cmd else cmd  # Just use last part for table
        stats_text += f"{cmd_short} | {len(times)} | {mean_val:.4f} | {median_val:.4f} | {std_val:.4f}\n"
    
    # Add text box with statistics
    plt.figtext(0.15, 0.01, stats_text, fontsize=9, 
                bbox=dict(boxstyle="round", facecolor="white", alpha=0.8),
                verticalalignment='bottom', horizontalalignment='left',
                family='monospace')
    
    plt.tight_layout(rect=[0, 0.2, 1, 1])  # Make room for the table at the bottom
    plt.savefig(os.path.join(output_dir, "combined_boxplot.png"), dpi=300)
    plt.close()

def main():
    parser = argparse.ArgumentParser("Plot title")
    parser.add_argument("title", type=str, help="Plot title")
    # Input file path
    log_file = "logs/kbot.log"
    
    # Parse log to DataFrame
    df = log_to_dataframe(log_file)
    df = df_by_type(df)

    custom_title = parser.parse_args().title 

    # Create output directory with custom title prefix
    output_dir = f"{custom_title}_plots"
    os.makedirs(output_dir, exist_ok=True)
    os.makedirs(os.path.join(output_dir, 'detailed'), exist_ok=True)  # Make detailed directory

    shutil.copy("logs/kbot.log", os.path.join(output_dir, "kbot.log"))

    output_file = os.path.join(output_dir, "kbot_parsed.csv")
    df.to_csv(output_file, index=False)
    print(f"Saved parsed data to {output_file}")

    # Pass custom_title to plotting functions
    plot_histograms(df, output_dir, custom_title)
    plot_detailed(df, output_dir, custom_title)
    # Create summary bar chart of command execution times
    plot_command_summary(df, output_dir, custom_title)
    
    # Create combined histogram of event time deltas
    plot_combined_histograms(df, output_dir, custom_title)
    
    # Create combined box plot
    plot_combined_boxplot(df, output_dir, custom_title)
    
    print(f"Generated plots in {output_dir} directory")

if __name__ == "__main__":
    main()

