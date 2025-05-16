import pandas as pd
import re

import re
import matplotlib.pyplot as plt
import os
import numpy as np

import argparse

# Timestamp pattern (ISO format)
TIMESTAMP_REGEX = re.compile(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)")

# Print type pattern (after timestamp and spaces)
PRINT_TYPE_REGEX = re.compile(r"Z\s+([^\s]+)")

# Thread ID pattern
THREAD_ID_REGEX = re.compile(r"ThreadId\((\d+)\)")


# EVENT_REGEX = re.compile(r"((?:(?!::(?:START|END)).)+?)::(START|END)")
# Event pattern (match everything after ThreadId including START/END if present)
EVENT_REGEX = re.compile(r"ThreadId\(\d+\)\s+(.*?(?:::START|::END|$))")

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
    df['command_type'] = df['event'].apply(
        lambda x: command_type_regex.search(x).group(0) if command_type_regex.search(x) else None
    )
    
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
                
                if elapsed_time > 0.1:
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
        max_time = np.max(times)
        x_limit = max(1.0, max_time * 1.1)  # Ensure at least 0-1ms range
        
        # Create bins from 0 to max value
        num_bins = min(200, max(91, int(x_limit*10)+1))  # Reasonable number of bins
        bins = np.linspace(0, x_limit, num_bins)
        
        # Plot histogram
        plt.hist(times, bins=bins, color='skyblue', edgecolor='black')
        plt.yscale('log')  # Keep y-axis as log scale
        plt.ylabel('Frequency (Log Scale)')
        
        # Set x-axis limit
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
            
            plt.axvline(mean_val, color='red', linestyle='dashed', linewidth=1, 
                       label=f'Mean: {mean_val:.4f}ms')
            plt.axvline(median_val, color='green', linestyle='dashed', linewidth=1, 
                       label=f'Median: {median_val:.4f}ms')
            
            plt.legend()
        
        plt.tight_layout()
        safe_command = command.replace('::', '_').replace('/', '_')
        plt.savefig(os.path.join(output_dir, f"{safe_command}_histogram.png"))
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

def main():
    parser = argparse.ArgumentParser("Plot title")
    parser.add_argument("title", type=str, help="Plot title")
    # Input file path
    log_file = "logs/kbot.log"
    
    # Parse log to DataFrame
    df = log_to_dataframe(log_file)
    df = df_by_type(df)
    
    # Save to CSV
    # output_file = "kbot_parsed.csv"
    # df.to_csv(output_file, index=False)
    # print(f"Saved parsed data to {output_file}")

    custom_title = parser.parse_args().title 

    # Create output directory with custom title prefix
    output_dir = f"{custom_title}_plots"
    os.makedirs(output_dir, exist_ok=True)

    # Pass custom_title to plotting functions
    plot_histograms(df, output_dir, custom_title)
    
    # Create summary bar chart of command execution times
    plot_command_summary(df, output_dir, custom_title)
    
    # Create combined histogram of event time deltas
    plot_combined_histograms(df, output_dir, custom_title)
    
    print(f"Generated plots in {output_dir} directory")

if __name__ == "__main__":
    main()

