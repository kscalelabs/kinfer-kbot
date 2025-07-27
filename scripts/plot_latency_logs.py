import pandas as pd
import re
import matplotlib.pyplot as plt
import os
import numpy as np
import shutil
import argparse


TIMESTAMP_REGEX = re.compile(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)")
PRINT_TYPE_REGEX = re.compile(r"Z\s+([^\s]+)")
THREAD_ID_REGEX = re.compile(r"ThreadId\((\d+)\)")
EVENT_REGEX = re.compile(r"ThreadId\(\d+\)\s+kinfer_kbot::[^:]+: src/[^:]+:\d+: ([^\s,]+)")
UUID_REGEX = re.compile(r"uuid=([0-9a-fA-F]{8}-(?:[0-9a-fA-F]{4}-){3}[0-9a-fA-F]{12})")



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


def find_outliers(df, output_dir=None):
    """
    Find instances where the main control loop elapsed time is more than 2 seconds off from the average.
    Save outliers to a text file if output_dir is provided.
    """
    # Filter for main control loop events that have elapsed time data
    main_loop_data = df[
        (df['command_type'] == 'runtime::main_control_loop::') & 
        (df['event_elapsed_time'].notna())
    ].copy()
    
    if len(main_loop_data) == 0:
        print("No main control loop events with elapsed time data found.")
        return
    
    # Calculate statistics
    elapsed_times = main_loop_data['event_elapsed_time'].values
    mean_time = np.mean(elapsed_times)
    std_time = np.std(elapsed_times)
    
    stats_text = f"\nMain Control Loop Timing Statistics:\n"
    stats_text += f"Total events: {len(elapsed_times)}\n"
    stats_text += f"Mean elapsed time: {mean_time:.6f} seconds\n"
    stats_text += f"Standard deviation: {std_time:.6f} seconds\n"
    stats_text += f"Min elapsed time: {np.min(elapsed_times):.6f} seconds\n"
    stats_text += f"Max elapsed time: {np.max(elapsed_times):.6f} seconds\n"
    stats_text += f"Min timestamp: {main_loop_data['timestamp'].min()}\n"
    
    print(stats_text)
    
    # Find outliers (more than 5ms off from average)
    threshold = 5*(10**-3)  # 5ms
    outlier_mask = np.abs(elapsed_times - mean_time) > threshold
    outliers = main_loop_data[outlier_mask].copy()
    
    if len(outliers) == 0:
        no_outliers_text = f"\nNo outliers found (events more than {threshold} seconds from mean)\n"
        print(no_outliers_text)
        
        # Still save the statistics even if no outliers
        if output_dir:
            outliers_file = os.path.join(output_dir, "outliers_analysis.txt")
            with open(outliers_file, 'w') as f:
                f.write("MAIN CONTROL LOOP OUTLIER ANALYSIS\n")
                f.write("=" * 50 + "\n")
                f.write(stats_text)
                f.write(no_outliers_text)
            print(f"Outlier analysis saved to {outliers_file}")
        return
    
    outliers_text = f"\nFound {len(outliers)} outliers (more than {threshold} seconds from mean of {mean_time:.6f}s):\n"
    outliers_text += "=" * 80 + "\n"
    
    print(outliers_text)
    
    # Sort outliers by how far they deviate from the mean
    outliers['deviation_from_mean'] = np.abs(outliers['event_elapsed_time'] - mean_time)
    outliers_sorted = outliers.sort_values('deviation_from_mean', ascending=False)
    
    detailed_outliers_text = ""
    for idx, row in outliers_sorted.iterrows():
        deviation = row['deviation_from_mean']
        elapsed = row['event_elapsed_time']
        iteration = row.get('iteration', 'N/A')
        timestamp = row['timestamp']
        uuid = row['uuid']
        
        outlier_detail = f"Iteration {iteration}:\n"
        outlier_detail += f"  Timestamp: {timestamp}\n"
        outlier_detail += f"  UUID: {uuid}\n"
        outlier_detail += f"  Elapsed time: {elapsed:.6f}s\n"
        outlier_detail += f"  Deviation from mean: {deviation/mean_time*100:.1f}% of mean\n"
        outlier_detail += "-" * 40 + "\n"
        
        print(outlier_detail)
        detailed_outliers_text += outlier_detail
    
    # Additional analysis
    high_outliers = outliers[outliers['event_elapsed_time'] > mean_time]
    low_outliers = outliers[outliers['event_elapsed_time'] < mean_time]
    
    analysis_text = f"\nOutlier Analysis:\n"
    analysis_text += f"Percentage of events that are outliers: {len(outliers)/len(elapsed_times)*100:.2f}%\n"
    analysis_text += f"High outliers (above mean): {len(high_outliers)}\n"
    analysis_text += f"Low outliers (below mean): {len(low_outliers)}\n"
    
    if len(high_outliers) > 0:
        analysis_text += f"Highest outlier: {np.max(high_outliers['event_elapsed_time']):.6f}s\n"
    if len(low_outliers) > 0:
        analysis_text += f"Lowest outlier: {np.min(low_outliers['event_elapsed_time']):.6f}s\n"
    
    print(analysis_text)
    
    # Save to file if output directory is provided
    if output_dir:
        outliers_file = os.path.join(output_dir, "outliers_analysis.txt")
        with open(outliers_file, 'w') as f:
            f.write("MAIN CONTROL LOOP OUTLIER ANALYSIS\n")
            f.write("=" * 50 + "\n")
            f.write(stats_text)
            f.write(outliers_text)
            f.write(detailed_outliers_text)
            f.write(analysis_text)
        
        print(f"Outlier analysis saved to {outliers_file}")

def plot_main_histogram(ax, filtering_stats, bins, command_types):
    for command in command_types:
        if command not in filtering_stats:
            continue
        stats = filtering_stats[command]
        filtered_times = stats['filtered_times']
        ax.hist(filtered_times, bins=bins, alpha=0.3, label=command)
    ax.set_yscale('log')
    ax.set_ylabel('Frequency (Log Scale)')
    ax.grid(axis='y', alpha=0.75, which='both')
    ax.legend(loc='upper center', fontsize='small')

def plot_zoomed_histogram(ax, filtering_stats, command_types, zoom_min, zoom_max, zoom_bins):
    ax.set_xlim(zoom_min, zoom_max)
    filtered_commands = []
    for cmd in command_types:
        if cmd not in filtering_stats:
            continue
        filtered_times = filtering_stats[cmd]['filtered_times']
        zoom_times = filtered_times[(filtered_times >= zoom_min) & (filtered_times <= zoom_max)]
        if len(zoom_times) > 0:
            filtered_commands.append((cmd, zoom_times))
    filtered_commands.sort(key=lambda x: np.median(x[1]))
    if filtered_commands:
        num_cmds = len(filtered_commands)
        offsets = np.linspace(-0.4, 0.4, num_cmds)
        width = 0.8 / num_cmds
        for i, (command, zoom_times) in enumerate(filtered_commands):
            counts, edges = np.histogram(zoom_times, bins=zoom_bins)
            centers = (edges[:-1] + edges[1:]) / 2
            ax.bar(centers + offsets[i] * width, counts, width=width * 0.9, alpha=0.8, label=command)
    ax.set_yscale('log')
    ax.set_xlabel('Time Since main_control_loop START (milliseconds)')
    ax.set_ylabel('Frequency (Log Scale)')
    ax.set_title(f'Zoom View: {zoom_min}-{zoom_max}ms Range (Filtered Data)')
    ax.grid(axis='y', alpha=0.75, which='both')
    if len(filtered_commands) > 10:
        ax.legend(loc='upper right', bbox_to_anchor=(0.95, 1), fontsize='small', ncol=2)
    else:
        ax.legend(loc='upper right', bbox_to_anchor=(0.95, 1))

def plot_outliers(ax, df):
    """
    Find slow main_control_loop iterations and create a stacked bar chart showing
    the breakdown of time spent in different events for each outlier iteration.
    """
    threshold_ms = 21.0

    # Which main_control_loop iterations are slow?
    mask_outlier_cl_end = (
        (df['event'] == 'runtime::main_control_loop::END') &
        (df['relative_to_start'] * 1000 > threshold_ms)
    )
    outlier_rows = df[mask_outlier_cl_end]

    if outlier_rows.empty:
        ax.text(0.5, 0.5, "No main_control_loop events > threshold",
                ha='center', va='center', fontsize=14)
        ax.axis('off')
        return []
    
    # Get all outlier UUIDs at once
    outlier_uuids = set(outlier_rows['uuid'].tolist())
    
    # Single mask to get all START events for outlier UUIDs
    start_events_mask = (
        (df['event'] == 'runtime::main_control_loop::START') &
        (df['uuid'].isin(outlier_uuids))
    )
    start_events = df[start_events_mask].set_index('uuid')
    
    # Group events by iteration for all outlier iterations at once
    outlier_dataframes = []
    
    for _, end_event in outlier_rows.iterrows():
        end_uuid = end_event['uuid']
        end_iteration = end_event['iteration']
        
        if end_uuid not in start_events.index:
            print(f"Warning: No START event found for END event with UUID {end_uuid}")
            continue
            
        start_event = start_events.loc[end_uuid]
        
        # Get all events in this iteration between START and END timestamps
        iteration_events_mask = (
            (df['iteration'] == end_iteration) &
            (df['timestamp'] >= start_event['timestamp']) &
            (df['timestamp'] <= end_event['timestamp'])
        )
        
        events_in_iteration = df[iteration_events_mask].sort_values('timestamp').copy()
        outlier_dataframes.append(events_in_iteration)
    
    # Create stacked bar chart
    if not outlier_dataframes:
        ax.text(0.5, 0.5, "No valid outlier iterations found",
                ha='center', va='center', fontsize=14)
        ax.axis('off')
        return outlier_dataframes
    
    # Prepare data for stacked bar chart
    bar_data = []
    bar_labels = []
    total_times = []
    
    for i, iteration_df in enumerate(outlier_dataframes):
        # Get the main control loop total time
        main_start = iteration_df[iteration_df['event'] == 'runtime::main_control_loop::START']
        main_end = iteration_df[iteration_df['event'] == 'runtime::main_control_loop::END']
        
        if main_start.empty or main_end.empty:
            continue
            
        total_time_ms = (main_end.iloc[0]['timestamp'] - main_start.iloc[0]['timestamp']).total_seconds() * 1000
        iteration_num = main_start.iloc[0]['iteration']
        
        # Calculate time breakdown for events with elapsed time data
        # EXCLUDE the main_control_loop itself since that's the total
        event_times = {}
        events_with_time = iteration_df[iteration_df['event_elapsed_time'].notna()]
        
        for _, event in events_with_time.iterrows():
            event_name = event['event']
            if event_name.endswith('::END') and not event_name.startswith('runtime::main_control_loop::'):
                # Get the base event name (remove ::END)
                base_event = event_name.replace('::END', '')
                elapsed_time_ms = event['event_elapsed_time'] * 1000
                event_times[base_event] = elapsed_time_ms
        
        # Calculate unaccounted time (time not attributed to any specific sub-event)
        accounted_time = sum(event_times.values())
        unaccounted_time = max(0, total_time_ms - accounted_time)
        
        if unaccounted_time > 0:
            event_times['unaccounted'] = unaccounted_time
        
        bar_data.append(event_times)
        bar_labels.append(f"Iter {iteration_num}\n({total_time_ms:.1f}ms)")
        total_times.append(total_time_ms)
    
    if not bar_data:
        ax.text(0.5, 0.5, "No events with timing data found",
                ha='center', va='center', fontsize=14)
        ax.axis('off')
        return outlier_dataframes
    
    # Get all unique event types across all iterations (excluding main_control_loop)
    all_events = set()
    for events in bar_data:
        all_events.update(events.keys())
    all_events = sorted(list(all_events))
    
    # Create color map for events
    colors = plt.cm.Set3(np.linspace(0, 1, len(all_events)))
    color_map = dict(zip(all_events, colors))
    
    # Create stacked bars
    x_positions = range(len(bar_data))
    bottom_values = np.zeros(len(bar_data))
    
    for event in all_events:
        heights = []
        for events in bar_data:
            heights.append(events.get(event, 0))
        
        ax.bar(x_positions, heights, bottom=bottom_values, 
               label=event, color=color_map[event], alpha=0.8)
        bottom_values += np.array(heights)
    
    # Customize the plot
    ax.set_xlabel('Outlier Iterations')
    ax.set_ylabel('Time (milliseconds)')
    ax.set_title(f'Time Breakdown for Slow Control Loop Iterations (>{threshold_ms}ms)')
    ax.set_xticks(x_positions)
    ax.set_xticklabels(bar_labels, rotation=45, ha='right')
    
    # Add legend
    if len(all_events) > 10:
        ax.legend(bbox_to_anchor=(1.05, 1), loc='upper left', fontsize='small')
    else:
        ax.legend(bbox_to_anchor=(1.05, 1), loc='upper left')
    
    # Add grid for better readability
    ax.grid(axis='y', alpha=0.3)
    
    # Add total time annotations on top of bars (should match the main_control_loop time)
    for i, (pos, calculated_total, actual_total) in enumerate(zip(x_positions, bottom_values, total_times)):
        ax.annotate(f'{actual_total:.1f}ms', 
                   xy=(pos, actual_total), 
                   xytext=(0, 3), 
                   textcoords='offset points',
                   ha='center', va='bottom',
                   fontsize=8, fontweight='bold')
        
        # Verify that our stacked components add up to the main control loop time
        if abs(calculated_total - actual_total) > 0.1:  # Allow small floating point differences
            print(f"Warning: Iteration {bar_labels[i]} - calculated total ({calculated_total:.1f}ms) "
                  f"doesn't match main_control_loop time ({actual_total:.1f}ms)")
    
    return outlier_dataframes

def plot_performance(df, output_dir, custom_title):
    command_types = df['event'].unique()
    fig, (ax1, ax2, ax3) = plt.subplots(3, 1, figsize=(20, 15), gridspec_kw={'height_ratios': [2, 1, 2]})

    all_times = df['relative_to_start'].values * 1000
    if len(all_times) == 0:
        print("Warning: No relative timing data available")
        return

    zoom_min, zoom_max = 18, 22
    zoom_bins = np.linspace(zoom_min, zoom_max, 40)
    timeout_threshold_ms = 30.0

    total_original_events = 0
    total_timeout_filtered = 0
    total_remaining_events = 0
    filtering_stats = {}
    all_filtered_times = []

    for command in command_types:
        cmd_data = df[df['event'] == command]
        if len(cmd_data) < 2:
            continue
        times = cmd_data['relative_to_start'].values * 1000
        original_count = len(times)
        total_original_events += original_count
        timeout_mask = times > timeout_threshold_ms
        timeout_filtered_count = np.sum(timeout_mask)
        filtered_times = times[~timeout_mask]
        remaining_count = len(filtered_times)
        filtering_stats[command] = {
            'original': original_count,
            'timeout_filtered': timeout_filtered_count,
            'remaining': remaining_count,
            'timeout_percentage': (timeout_filtered_count / original_count * 100) if original_count > 0 else 0,
            'filtered_times': filtered_times
        }
        total_timeout_filtered += timeout_filtered_count
        total_remaining_events += remaining_count
        all_filtered_times.extend(filtered_times)

    if len(all_filtered_times) > 0:
        filtered_max_time = np.max(all_filtered_times)
        filtered_min_time = np.min(all_filtered_times)
        x_limit = max(1.0, filtered_max_time * 1.05)
        x_min = max(0.0, filtered_min_time * 0.95)
        num_bins = min(200, max(50, int((x_limit - x_min) * 10) + 1))
        bins = np.linspace(x_min, x_limit, num_bins)
    else:
        x_limit = 1.0
        x_min = 0.0
        bins = np.linspace(0, 1, 50)

    # Call subfunctions for each axis
    plot_main_histogram(ax1, filtering_stats, bins, command_types)
    ax1.set_xlim(x_min, x_limit)
    title_with_filtering = f'{custom_title} - Event Timing Relative to Control Loop Start'
    title_with_filtering += f'\n(Timeout: {total_timeout_filtered} filtered, {total_remaining_events} remaining)'
    ax1.set_title(title_with_filtering)

    plot_zoomed_histogram(ax2, filtering_stats, command_types, zoom_min, zoom_max, zoom_bins)
    plot_outliers(ax3, df)

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

    find_outliers(df, output_dir)

    plot_performance(df, output_dir, custom_title)

    shutil.copy(f"logs/{parser.parse_args().file_path}", os.path.join(output_dir, f"{custom_title}_{parser.parse_args().file_path}"))

    output_file = os.path.join(output_dir, "kbot_parsed.csv")
    df.to_csv(output_file, index=False)
    print(f"Saved parsed data to {output_file}")
    
    print(f"Generated plots in {output_dir} directory")

if __name__ == "__main__":
    main()

