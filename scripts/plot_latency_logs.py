import pandas as pd
import re
import matplotlib.pyplot as plt
import os
import numpy as np
import shutil
import argparse
import matplotlib.colors as mcolors


TIMESTAMP_REGEX = re.compile(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)")
PRINT_TYPE_REGEX = re.compile(r"Z\s+([^\s]+)")
THREAD_ID_REGEX = re.compile(r"ThreadId\((\d+)\)")
EVENT_REGEX = re.compile(r"ThreadId\(\d+\)\s+kinfer_kbot::[^:]+: src/[^:]+:\d+: ([^\s,]+)")
UUID_REGEX = re.compile(r"uuid=([0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12})")
# ELAPSED_REGEX = re.compile(r"elapsed: Ok\(([^)]+)\)")

def log_to_dataframe(log_file):
    print(f"Parsing log file: {log_file}")
    data = []
    with open(log_file, 'r', encoding='utf-8', errors='ignore') as f:
        for line in f:
            timestamp_match = TIMESTAMP_REGEX.search(line)
            event_match = EVENT_REGEX.search(line)
            print_type_match = PRINT_TYPE_REGEX.search(line)
            thread_id_match = THREAD_ID_REGEX.search(line)
            uuid_match = UUID_REGEX.search(line)

            timestamp = timestamp_match.group(1) if timestamp_match else None
            event = event_match.group(1) if event_match else None
            print_type = print_type_match.group(1) if print_type_match else None
            thread_id = thread_id_match.group(0) if thread_id_match else None
            uuid = uuid_match.group(1) if uuid_match else None

            if print_type == "DEBUG":
                data.append({
                    "timestamp": timestamp,
                    "event": event,
                    "print_type": print_type,
                    "thread_id": thread_id,
                    "uuid": uuid,
                })

    df = pd.DataFrame(data)
    print(f"Parsed {len(df)} log entries")

    command_type_regex = re.compile(r'(provider|runtime)::([^:]+)::')
    df['command_type'] = df['event'].apply(
        lambda x: f"{command_type_regex.search(x).group(1)}::{command_type_regex.search(x).group(2)}::" 
        if command_type_regex.search(x) else None
    )

    return df

def df_calc_rel_start(df):
    df = df.copy()
    df['timestamp'] = pd.to_datetime(df['timestamp'])
    starts = df.loc[df['event'] == 'runtime::main_control_loop::START', 'timestamp'].reset_index(drop=True)
    
    edges = list(starts) + [df['timestamp'].max() + pd.Timedelta(microseconds=1)]
    df['iteration'] = pd.cut(
        df['timestamp'],
        bins=edges,
        labels=range(1, len(starts)+1),
        right=False
    )

    df = df.loc[df['iteration'].notna()].copy()
    if len(df) == 0:
        print("Warning: No events could be matched to iterations")
        return df
        
    df['iteration'] = df['iteration'].astype(int)
    
    df['relative_to_start'] = df.apply(
        lambda r: (r['timestamp'] - starts[r['iteration'] - 1]).total_seconds(),
        axis=1
    )

    return df

def df_calc_pair(df):
    df = df.copy()
    starts = df[df['event'].str.endswith('START')]
    ends = df[df['event'].str.endswith('END')]
    
    start_dict = {row['uuid']: row for _, row in starts.iterrows()}
    
    for idx, end_event in ends.iterrows():
        uuid = end_event['uuid']
        if uuid in start_dict:
            start_event = start_dict[uuid]
            
            elapsed_time = (end_event['timestamp'] - start_event['timestamp']).total_seconds()
            
            df.at[idx, 'event_elapsed_time'] = elapsed_time
    
    return df

def plot_histograms(df, output_dir, custom_title):
    command_types = df['command_type'].unique()
    
    for command in command_types:
        cmd_data = df[df['command_type'] == command]
        if len(cmd_data) < 2:  # Skip if too few data points
            continue
            
        times = cmd_data['event_elapsed_time'].values * 1000
        
        plt.figure(figsize=(15, 7))
        
        bin_size = 0.1  # 0.1ms bin size
        x_limit = 22  # 22ms limit
        bins = np.arange(0, x_limit + bin_size, bin_size)

        plt.hist(times, bins=bins, color='skyblue', edgecolor='black')
        plt.yscale('log')  # Keep y-axis as log scale
        plt.ylabel('Frequency (Log Scale)')
        
        plt.xlim(0, x_limit)
        
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
            plt.text(0.95, 0.95, f"Var: {variance_val:.4f}, Std: {std_val:.4f} \n Bin Size: {bin_size:.4f}. \n Plot truncated at 22ms, but metrics includes.",
            transform=plt.gca().transAxes,
            verticalalignment='top', horizontalalignment='right',
            fontsize=9, bbox=dict(boxstyle="round", facecolor="white", alpha=0.7))

            
            plt.legend()
        
        plt.tight_layout()
        safe_command = command.replace('::', '_').replace('/', '_')
        plt.savefig(os.path.join(output_dir, f"{safe_command}_histogram.png"))
        plt.close()


def plot_combined_histograms(df, output_dir, custom_title):

    command_types = df['command_type'].unique()
    df = df.dropna(subset=['event_elapsed_time'])
    
    plt.figure(figsize=(15, 7))
    
    bin_size = 0.1  # 0.1ms bin size
    x_limit = 22  # 22ms limit
    bins = np.arange(0, x_limit + bin_size, bin_size)
    
    for command in command_types:
        cmd_data = df[df['command_type'] == command]
        if len(cmd_data) < 2:  # Skip if too few data points
            continue
        
        times = cmd_data['event_elapsed_time'].values * 1000
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

# ---------------------------------------------------------------------
# helper – distinct colours
def _distinct_colors(n, sat=0.8, val=0.95):
    hues = np.linspace(0, 1, n, endpoint=False)
    return [mcolors.hsv_to_rgb((h, sat, val)) for h in hues]

# ---------------------------------------------------------------------
def plot_timeline_grouped(
    df: pd.DataFrame,
    output_dir: str,
    custom_title: str | None = None,
    point_size: float = 4,
):
    # 1 ── timestamps
    df = df.copy()
    df["ts"] = pd.to_datetime(df["timestamp"])

    # 2 ── drop pre-loop rows
    first_start = df.loc[
        df["event"] == "runtime::main_control_loop::START", "ts"
    ].min()
    df = df[df["ts"] >= first_start]

    # 3 ── iteration + offset (µs)
    starts = df.loc[
        df["event"] == "runtime::main_control_loop::START", "ts"
    ].to_numpy()
    df["iteration"] = np.searchsorted(starts, df["ts"].to_numpy(), side="right")
    df["offset_us"] = (
        (df["ts"] - starts[df["iteration"] - 1]) / pd.Timedelta(microseconds=1)
    )


    # 4 ── manual 8-panel grouping
    groups = {
        # ❶ – provider::trigger_actuator_read
        "trigger_actuator_read": [
            "provider::trigger_actuator_read::START",
            "provider::trigger_actuator_read::END",
        ],

        # ❷ – provider::get_actuator_state
        "get_actuator_state": [
            "provider::get_actuator_state::START",
            "provider::get_actuator_state::END",
        ],

        # ❸ – provider::get_joint_angles
        "get_joint_angles": [
            "provider::get_joint_angles::START",
            "provider::get_joint_angles::END",
        ],

        # ❹ – provider::get_joint_angular_velocities
        "get_joint_angular_velocities": [
            "provider::get_joint_angular_velocities::START",
            "provider::get_joint_angular_velocities::END",
        ],

        # ❺ – provider::get_projected_gravity
        "get_projected_gravity": [
            "provider::get_projected_gravity::START",
            "provider::get_projected_gravity::END",
        ],

        # ❻ – runtime::model_runner_step
        "model_runner_step": [
            "runtime::model_runner_step::START",
            "runtime::model_runner_step::END",
        ],

        # ❼ – runtime::main_control_loop
        "main_control_loop": [
            "runtime::main_control_loop::START",
            "runtime::main_control_loop::END",
        ],

        # ❽ – ALL events (explicit, no wild-cards)
        "ALL START vs ALL END": [
            # START events
            "provider::trigger_actuator_read::START",
            "provider::get_actuator_state::START",
            "provider::get_joint_angles::START",
            "provider::get_joint_angular_velocities::START",
            "provider::get_projected_gravity::START",
            "runtime::model_runner_step::START",
            "runtime::main_control_loop::START",

            # END events
            "provider::trigger_actuator_read::END",
            "provider::get_actuator_state::END",
            "provider::get_joint_angles::END",
            "provider::get_joint_angular_velocities::END",
            "provider::get_projected_gravity::END",
            "runtime::model_runner_step::END",
            "runtime::main_control_loop::END",
        ],
    }

    # 5 ── warn about unmatched events
    unmatched = set(df["event"].unique())
    for pats in groups.values():
        for pat in pats:
            unmatched = {ev for ev in unmatched if pat not in ev}
    if unmatched:
        print("⚠️  Unmatched events (not plotted):")
        for ev in sorted(unmatched):
            print("   •", ev)

    # 6 ── figure & axes (4 rows × 2 cols, taller canvas)
    fig, axes = plt.subplots(4, 2, figsize=(18, 24), sharey=True)
    axes = axes.flatten()

    # colour palette large enough for every pattern separately
    palette = _distinct_colors(sum(len(v) for v in groups.values()))
    colour_iter = iter(palette)

    for ax, (title, patterns) in zip(axes, groups.items()):
        # build mask for this subplot
        mask = df["event"].apply(lambda ev: any(pat in ev for pat in patterns))
        if not mask.any():
            ax.text(0.5, 0.5, "no data", ha="center", va="center")
            ax.axis("off")
            continue
        
        total_outliers = 0
        
        for pat in patterns:
            sub = df[mask & df["event"].str.contains(pat)]
            if sub.empty:
                continue
                
            # Outlier removal using IQR method
            offsets = sub["offset_us"].values
            if len(offsets) > 10:  # Only remove outliers if we have enough data points
                q1, q3 = np.percentile(offsets, [25, 75])
                iqr = q3 - q1
                #! Outlier bound is 4 * IQR
                lower_bound = q1 - 4 * iqr
                upper_bound = q3 + 4 * iqr
                
                # Count outliers
                outlier_mask = (offsets < lower_bound) | (offsets > upper_bound)
                num_outliers = np.sum(outlier_mask)
                
                # Filter out outliers
                if num_outliers > 0:
                    total_outliers += num_outliers
                    sub = sub[~((sub["offset_us"] < lower_bound) | (sub["offset_us"] > upper_bound))]
            
            ax.scatter(
                sub["offset_us"],
                sub["iteration"],
                s=point_size,
                alpha=0.9,
                rasterized=True,
                label=pat,
            )
        
        # Update title with outlier info if any were removed
        panel_title = title
        if total_outliers > 0:
            panel_title += f" ({total_outliers} outliers removed)"
            
        ax.set_title(panel_title, fontsize="medium")
        ax.set_xlabel("Offset (µs)")
        ax.set_ylabel("Iteration #")
        ax.legend(
            loc="upper right",
            fontsize="x-small",
            frameon=True,
            facecolor='white',
            handlelength=1.0,
            handletextpad=0.4,
        )

    # 7 ── final styling & save
    fig.suptitle(
        f"{custom_title } \n Time Offset from Main Control Loop Start" or "Firmware Event Timeline — 8-panel view", fontsize=16
    )
    fig.tight_layout(rect=[0, 0.03, 1, 0.96])

    os.makedirs(output_dir, exist_ok=True)
    out_path = os.path.join(output_dir, "snake_plot_8panel.png")
    fig.savefig(out_path, dpi=300)
    plt.close(fig)
    print(f"✅  Saved 8-panel snake plot → {os.path.abspath(out_path)}")

def plot_relative_to_start(df, output_dir, custom_title):
    """
    Create a histogram showing when events occur relative to their iteration's main_control_loop::START.
    Events are grouped by command_type and colored differently.
    """
    command_types = df['event'].unique()
    
    # Create a figure with two subplots - main plot and zoomed region
    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(15, 10), gridspec_kw={'height_ratios': [2, 1]})
    
    # Convert relative times to milliseconds for better visualization
    all_times = df['relative_to_start'].values * 1000
    
    if len(all_times) == 0:
        print("Warning: No relative timing data available")
        return
    
    # Create bins for zoomed plot (18-22ms)
    zoom_min, zoom_max = 18, 22
    zoom_bins = np.linspace(zoom_min, zoom_max, 40)
    
    # Keep track of total outliers removed for ax1
    total_ax1_outliers = 0
    
    # Plot histograms for each command type on main subplot
    for i, command in enumerate(command_types):
        cmd_data = df[df['event'] == command]
        if len(cmd_data) < 2:  # Skip if too few data points
            continue
        
        # Convert to milliseconds
        times = cmd_data['relative_to_start'].values * 1000
        
        #* Apply outlier filtering for ax1 - DISABLED
        # if len(times) > 10:
        #     q1, q3 = np.percentile(times, [25, 75])
        #     iqr = q3 - q1
        #     # Set multiplier to very large number to effectively disable filtering
        #     lower_bound = q1 - 1000000 * iqr
        #     upper_bound = q3 + 1000000 * iqr
        #     
        #     outlier_mask = (times < lower_bound) | (times > upper_bound)
        #     num_outliers = np.sum(outlier_mask)
        #     total_ax1_outliers += num_outliers
        #     
        #     # Filter out outliers for plotting
        #     filtered_times = times[~outlier_mask]
        # else:
        #     filtered_times = times
        
        #* No filtering - use all data points
        filtered_times = times

        max_time = np.max(filtered_times)
        x_limit = max(1.0, max_time * 1.1)  # Ensure at least 0-1ms range
        
        # Create bins from 0 to max value for main plot
        num_bins = min(200, max(91, int(x_limit*10)+1))  # Reasonable number of bins
        bins = np.linspace(0, x_limit, num_bins)
    
        # Plot on main subplot with filtered data
        ax1.hist(filtered_times, bins=bins, alpha=0.3, label=command)
    
    # For zoomed view, use a different approach - plot each command type separately without overlap
    zoom_range = (zoom_min, zoom_max)
    ax2.set_xlim(*zoom_range)
    
    # Number of commands to display in zoomed view
    filtered_commands = []
    for cmd in command_types:
        cmd_data = df[df['event'] == cmd]
        times = cmd_data['relative_to_start'].values * 1000
        zoom_times = times[(times >= zoom_min) & (times <= zoom_max)]
        if len(zoom_times) > 0:
            filtered_commands.append((cmd, zoom_times))
    
    # Sort commands by their median time in the zoom range
    filtered_commands.sort(key=lambda x: np.median(x[1]))
    
    # For zoomed view, use side-by-side bars instead of overlapping histograms
    if filtered_commands:
        num_cmds = len(filtered_commands)
        offsets = np.linspace(-0.4, 0.4, num_cmds)
        width = 0.8 / num_cmds  # Adjust bar width based on command count
        
        for i, (command, zoom_times) in enumerate(filtered_commands):
            # Create histogram data
            counts, edges = np.histogram(zoom_times, bins=zoom_bins)
            # Plot as bars, slightly offset from each other
            centers = (edges[:-1] + edges[1:]) / 2
            idx = command_types.tolist().index(command)
            ax2.bar(centers + offsets[i] * width, counts, width=width * 0.9, 
                   alpha=0.8, label=command)
    
    # Configure main subplot
    ax1.set_yscale('log')  # Use log scale for y-axis
    ax1.set_xlim(0, x_limit)
    title_with_outliers = f'{custom_title} - Event Timing Relative to Control Loop Start'
    title_with_outliers += f' ({total_ax1_outliers} outliers removed)'
    ax1.set_title(title_with_outliers)
    ax1.set_ylabel('Frequency (Log Scale)')
    ax1.grid(axis='y', alpha=0.75, which='both')
    ax1.legend(loc='upper right', fontsize='small')
    
    # Configure zoomed subplot
    ax2.set_yscale('log')  # Use log scale for y-axis
    ax2.set_xlabel('Time Since main_control_loop START (milliseconds)')
    ax2.set_ylabel('Frequency (Log Scale)')
    ax2.set_title(f'Zoom View: {zoom_min}-{zoom_max}ms Range')
    ax2.grid(axis='y', alpha=0.75, which='both')
    
    # Handle legend - if many command types, make it more compact
    if len(filtered_commands) > 10:
        ax2.legend(loc='upper right', bbox_to_anchor=(0.95, 1), 
                   fontsize='small', ncol=2)
    else:
        ax2.legend(loc='upper right', bbox_to_anchor=(0.95, 1))
    
    plt.tight_layout()
    plt.savefig(os.path.join(output_dir, "relative_to_start_histogram.png"), dpi=300)
    plt.close()
    
    print(f"Relative timing histogram saved → {os.path.join(output_dir, 'relative_to_start_histogram.png')}")

def main():
    parser = argparse.ArgumentParser("Plot title")
    parser.add_argument("title", type=str, help="Plot title")
    parser.add_argument("file_path", type=str, help="Log file", default="kbot.log")
    # Input file path
    log_file = f"logs/{parser.parse_args().file_path}"

    custom_title = parser.parse_args().title 
    output_dir = f"{custom_title}_plots"
    os.makedirs(output_dir, exist_ok=True)


    df = log_to_dataframe(log_file)
    df = df_calc_rel_start(df)
    df = df_calc_pair(df)

    plot_histograms(df, output_dir, custom_title)
    plot_timeline_grouped(df, output_dir, custom_title)
    plot_relative_to_start(df, output_dir, custom_title)
    plot_combined_histograms(df, output_dir, custom_title)


    shutil.copy(f"logs/{parser.parse_args().file_path}", os.path.join(output_dir, f"{custom_title}_{parser.parse_args().file_path}"))

    output_file = os.path.join(output_dir, "kbot_parsed.csv")
    df.to_csv(output_file, index=False)
    print(f"Saved parsed data to {output_file}")
    
    print(f"Generated plots in {output_dir} directory")

if __name__ == "__main__":
    main()

